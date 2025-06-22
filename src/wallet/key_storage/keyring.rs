use anyhow::{anyhow, Result};
use async_trait::async_trait;
use keyring::Entry;
use secrecy::{ExposeSecret, SecretString};
use tracing::{debug, info};

use super::KeyStorage;

const DEFAULT_SERVICE: &str = "omikuji";

#[derive(Debug, Clone)]
pub struct KeyringStorage {
    service: String,
}

impl KeyringStorage {
    pub fn new(service: Option<String>) -> Self {
        Self {
            service: service.unwrap_or_else(|| DEFAULT_SERVICE.to_string()),
        }
    }

    fn get_entry(&self, network: &str) -> Result<Entry> {
        Entry::new(&self.service, network)
            .map_err(|e| anyhow!("Failed to create keyring entry: {}", e))
    }
}

#[async_trait]
impl KeyStorage for KeyringStorage {
    async fn get_key(&self, network: &str) -> Result<SecretString> {
        debug!("Retrieving key for network '{}' from keyring", network);

        let entry = self.get_entry(network)?;
        let password = entry
            .get_password()
            .map_err(|e| anyhow!("Failed to retrieve key for network '{}': {}", network, e))?;

        debug!("Successfully retrieved key for network '{}'", network);
        Ok(SecretString::from(password))
    }

    async fn store_key(&self, network: &str, key: SecretString) -> Result<()> {
        info!("Storing key for network '{}' in keyring", network);

        let entry = self.get_entry(network)?;
        entry
            .set_password(key.expose_secret())
            .map_err(|e| anyhow!("Failed to store key for network '{}': {}", network, e))?;

        info!("Successfully stored key for network '{}'", network);
        Ok(())
    }

    async fn remove_key(&self, network: &str) -> Result<()> {
        info!("Removing key for network '{}' from keyring", network);

        let entry = self.get_entry(network)?;
        entry
            .delete_credential()
            .map_err(|e| anyhow!("Failed to remove key for network '{}': {}", network, e))?;

        info!("Successfully removed key for network '{}'", network);
        Ok(())
    }

    async fn list_keys(&self) -> Result<Vec<String>> {
        // Note: The keyring crate doesn't support listing all keys directly
        // This would need to be tracked separately or use platform-specific APIs
        Err(anyhow!(
            "Listing keys is not directly supported by the keyring crate. \
            Consider maintaining a separate list of networks in configuration."
        ))
    }
}
