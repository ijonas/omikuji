use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use aws_config::{BehaviorVersion, Region};
use aws_sdk_secretsmanager::Client;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::KeyStorage;

/// Cache entry for a secret
#[derive(Clone)]
struct CacheEntry {
    secret: SecretString,
    cached_at: Instant,
}

/// AWS secret format
#[derive(Serialize, Deserialize)]
struct SecretData {
    private_key: String,
    network: String,
    created_at: String,
    created_by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

/// AWS Secrets Manager storage implementation
pub struct AwsSecretsStorage {
    client: Client,
    prefix: String,
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    cache_ttl: Duration,
}

impl AwsSecretsStorage {
    /// Create a new AWS Secrets Manager storage instance
    pub async fn new(
        region: Option<String>,
        prefix: &str,
        cache_ttl_seconds: Option<u64>,
    ) -> Result<Self> {
        // Build AWS config with best practices
        let mut config_builder = aws_config::defaults(BehaviorVersion::latest());

        // Set region if provided
        if let Some(region_str) = region {
            config_builder = config_builder.region(Region::new(region_str));
        }

        // Load config (will use IAM role, environment variables, or config files)
        let config = config_builder.load().await;
        let client = Client::new(&config);

        // Test connection by listing secrets with limit 1
        match client.list_secrets().max_results(1).send().await {
            Ok(_) => info!("Successfully connected to AWS Secrets Manager"),
            Err(e) => {
                error!("Failed to connect to AWS Secrets Manager: {}", e);
                // Don't fail here - let errors happen on actual operations
            }
        }

        Ok(Self {
            client,
            prefix: prefix.trim_end_matches('/').to_string(),
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: Duration::from_secs(cache_ttl_seconds.unwrap_or(300)), // Default 5 minutes
        })
    }

    /// Get the full secret name for a network
    fn get_secret_name(&self, network: &str) -> String {
        if self.prefix.is_empty() {
            network.to_string()
        } else {
            format!("{}/{}", self.prefix, network)
        }
    }

    /// Check if a cache entry is still valid
    fn is_cache_valid(&self, entry: &CacheEntry) -> bool {
        entry.cached_at.elapsed() < self.cache_ttl
    }

    /// Log audit event for key access
    fn audit_log(&self, operation: &str, network: &str, success: bool) {
        info!(
            target: "omikuji::audit",
            operation = operation,
            network = network,
            success = success,
            storage_type = "aws_secrets",
            timestamp = chrono::Utc::now().to_rfc3339(),
            "Key storage operation"
        );
    }

    /// Parse different secret formats
    fn parse_secret_value(&self, secret_string: &str, _network: &str) -> Result<String> {
        // First try to parse as JSON
        if let Ok(data) = serde_json::from_str::<SecretData>(secret_string) {
            return Ok(data.private_key);
        }

        // Try as a JSON object with various key names
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(secret_string) {
            if let Some(obj) = json.as_object() {
                // Try common key names
                for key in ["private_key", "privateKey", "key", "value"] {
                    if let Some(value) = obj.get(key) {
                        if let Some(key_str) = value.as_str() {
                            return Ok(key_str.to_string());
                        }
                    }
                }
            }
        }

        // If not JSON, treat the entire string as the key
        Ok(secret_string.to_string())
    }
}

#[async_trait]
impl KeyStorage for AwsSecretsStorage {
    async fn get_key(&self, network: &str) -> Result<SecretString> {
        debug!(
            "Retrieving key for network '{}' from AWS Secrets Manager",
            network
        );

        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get(network) {
                if self.is_cache_valid(entry) {
                    debug!("Returning cached key for network '{}'", network);
                    self.audit_log("get_key_cached", network, true);
                    return Ok(entry.secret.clone());
                }
            }
        }

        // Fetch from AWS
        let secret_name = self.get_secret_name(network);

        match self
            .client
            .get_secret_value()
            .secret_id(&secret_name)
            .send()
            .await
        {
            Ok(response) => {
                let secret_string = response.secret_string().ok_or_else(|| {
                    anyhow!("Secret for network '{}' has no string value", network)
                })?;

                let private_key = self.parse_secret_value(secret_string, network)?;
                let secret = SecretString::from(private_key);

                // Update cache
                {
                    let mut cache = self.cache.write().await;
                    cache.insert(
                        network.to_string(),
                        CacheEntry {
                            secret: secret.clone(),
                            cached_at: Instant::now(),
                        },
                    );
                }

                info!(
                    "Successfully retrieved key for network '{}' from AWS Secrets Manager",
                    network
                );
                self.audit_log("get_key", network, true);
                Ok(secret)
            }
            Err(e) => {
                warn!(
                    "Failed to retrieve key from AWS for network '{}': {}",
                    network, e
                );

                // Check cache for fallback
                let cache = self.cache.read().await;
                if let Some(entry) = cache.get(network) {
                    warn!(
                        "Using cached key for network '{}' due to AWS error",
                        network
                    );
                    self.audit_log("get_key_fallback", network, true);
                    return Ok(entry.secret.clone());
                }

                self.audit_log("get_key", network, false);
                Err(anyhow!(
                    "Failed to retrieve key for network '{}': {}",
                    network,
                    e
                ))
            }
        }
    }

    async fn store_key(&self, network: &str, key: SecretString) -> Result<()> {
        debug!(
            "Storing key for network '{}' in AWS Secrets Manager",
            network
        );

        let secret_name = self.get_secret_name(network);

        // Create structured secret data
        let secret_data = SecretData {
            private_key: key.expose_secret().to_string(),
            network: network.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            created_by: "omikuji".to_string(),
            description: Some(format!("Private key for Omikuji network: {}", network)),
        };

        let secret_string =
            serde_json::to_string(&secret_data).context("Failed to serialize secret data")?;

        // Try to update existing secret first
        match self
            .client
            .put_secret_value()
            .secret_id(&secret_name)
            .secret_string(&secret_string)
            .send()
            .await
        {
            Ok(_) => {
                // Update cache
                {
                    let mut cache = self.cache.write().await;
                    cache.insert(
                        network.to_string(),
                        CacheEntry {
                            secret: key,
                            cached_at: Instant::now(),
                        },
                    );
                }

                info!(
                    "Successfully updated key for network '{}' in AWS Secrets Manager",
                    network
                );
                self.audit_log("store_key", network, true);
                return Ok(());
            }
            Err(e) => {
                // If secret doesn't exist, try to create it
                if e.to_string().contains("ResourceNotFoundException") {
                    debug!(
                        "Secret doesn't exist, creating new secret for network '{}'",
                        network
                    );
                } else {
                    error!("Failed to update secret: {}", e);
                    self.audit_log("store_key", network, false);
                    return Err(anyhow!(
                        "Failed to store key for network '{}': {}",
                        network,
                        e
                    ));
                }
            }
        }

        // Create new secret
        match self
            .client
            .create_secret()
            .name(&secret_name)
            .description(format!("Omikuji private key for network: {}", network))
            .secret_string(&secret_string)
            .tags(
                aws_sdk_secretsmanager::types::Tag::builder()
                    .key("Application")
                    .value("omikuji")
                    .build(),
            )
            .tags(
                aws_sdk_secretsmanager::types::Tag::builder()
                    .key("Network")
                    .value(network)
                    .build(),
            )
            .send()
            .await
        {
            Ok(_) => {
                // Update cache
                {
                    let mut cache = self.cache.write().await;
                    cache.insert(
                        network.to_string(),
                        CacheEntry {
                            secret: key,
                            cached_at: Instant::now(),
                        },
                    );
                }

                info!(
                    "Successfully created key for network '{}' in AWS Secrets Manager",
                    network
                );
                self.audit_log("store_key", network, true);
                Ok(())
            }
            Err(e) => {
                error!("Failed to create secret for network '{}': {}", network, e);
                self.audit_log("store_key", network, false);
                Err(anyhow!(
                    "Failed to store key for network '{}': {}",
                    network,
                    e
                ))
            }
        }
    }

    async fn remove_key(&self, network: &str) -> Result<()> {
        debug!(
            "Removing key for network '{}' from AWS Secrets Manager",
            network
        );

        let secret_name = self.get_secret_name(network);

        // AWS best practice: schedule deletion instead of immediate deletion
        match self
            .client
            .delete_secret()
            .secret_id(&secret_name)
            .recovery_window_in_days(7) // 7-day recovery window
            .send()
            .await
        {
            Ok(_) => {
                // Remove from cache
                {
                    let mut cache = self.cache.write().await;
                    cache.remove(network);
                }

                info!("Successfully scheduled deletion of key for network '{}' (7-day recovery window)", network);
                self.audit_log("remove_key", network, true);
                Ok(())
            }
            Err(e) => {
                error!(
                    "Failed to delete key from AWS for network '{}': {}",
                    network, e
                );
                self.audit_log("remove_key", network, false);
                Err(anyhow!(
                    "Failed to remove key for network '{}': {}",
                    network,
                    e
                ))
            }
        }
    }

    async fn list_keys(&self) -> Result<Vec<String>> {
        debug!("Listing keys from AWS Secrets Manager");

        let mut networks = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut request = self.client.list_secrets();

            // Add filters for our prefix
            if !self.prefix.is_empty() {
                request = request.filters(
                    aws_sdk_secretsmanager::types::Filter::builder()
                        .key(aws_sdk_secretsmanager::types::FilterNameStringType::Name)
                        .values(&self.prefix)
                        .build(),
                );
            }

            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            match request.send().await {
                Ok(response) => {
                    if let Some(secret_list) = response.secret_list {
                        for secret in secret_list {
                            if let Some(name) = secret.name() {
                                // Extract network name from full secret name
                                let network_name =
                                    if !self.prefix.is_empty() && name.starts_with(&self.prefix) {
                                        name.strip_prefix(&self.prefix)
                                            .unwrap_or(name)
                                            .trim_start_matches('/')
                                            .to_string()
                                    } else {
                                        name.to_string()
                                    };

                                // Skip if it's marked for deletion
                                if secret.deleted_date().is_none() {
                                    networks.push(network_name);
                                }
                            }
                        }
                    }

                    // Check if there are more results
                    next_token = response.next_token;
                    if next_token.is_none() {
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to list keys from AWS: {}", e);
                    self.audit_log("list_keys", "", false);
                    return Err(anyhow!("Failed to list keys: {}", e));
                }
            }
        }

        info!("Found {} keys in AWS Secrets Manager", networks.len());
        self.audit_log("list_keys", "", true);
        Ok(networks)
    }
}

// Clean up expired cache entries periodically
impl AwsSecretsStorage {
    pub async fn start_cache_cleanup(&self) {
        let cache = self.cache.clone();
        let cache_ttl = self.cache_ttl;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60)); // Check every minute
            loop {
                interval.tick().await;
                let mut cache = cache.write().await;
                let now = Instant::now();
                cache.retain(|_, entry| now.duration_since(entry.cached_at) < cache_ttl);
            }
        });
    }
}

#[cfg(test)]
#[path = "aws_secrets_tests.rs"]
mod tests;
