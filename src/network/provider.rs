use std::collections::HashMap;
use std::sync::Arc;

use alloy::{
    primitives::Address,
    providers::{Provider, ProviderBuilder, RootProvider},
    signers::local::PrivateKeySigner,
    transports::http::{Client, Http},
};
use anyhow::{Context, Result};
use secrecy::ExposeSecret;
use thiserror::Error;
use tracing::{error, info};
use url::Url;

use crate::config::models::Network;
use crate::wallet::key_storage::KeyStorage;

/// Errors that can occur when interacting with network providers
#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("Network not found: {0}")]
    NetworkNotFound(String),

    #[error("Provider error: {0}")]
    #[allow(dead_code)]
    ProviderError(String),

    #[error("RPC connection failed: {0}")]
    ConnectionFailed(String),
}

/// Type alias for the alloy provider we will use
pub type EthProvider = RootProvider<Http<Client>>;

/// Manages the connections to different EVM networks
pub struct NetworkManager {
    /// Map of network name to provider
    providers: HashMap<String, Arc<EthProvider>>,

    /// Private keys for each network (stored securely)
    private_keys: HashMap<String, String>,

    /// RPC URLs for each network (needed for creating signed providers)
    rpc_urls: HashMap<String, String>,

    /// Wallet addresses for each network
    wallet_addresses: HashMap<String, Address>,
}

impl NetworkManager {
    /// Create a new network manager from a list of network configurations
    pub async fn new(networks: &[Network]) -> Result<Self> {
        let mut providers = HashMap::new();
        let private_keys = HashMap::new();
        let mut rpc_urls = HashMap::new();
        let wallet_addresses = HashMap::new();

        for network in networks {
            let provider = Self::create_provider(&network.rpc_url)
                .await
                .with_context(|| {
                    format!("Failed to create provider for network {}", network.name)
                })?;

            providers.insert(network.name.clone(), Arc::new(provider));
            rpc_urls.insert(network.name.clone(), network.rpc_url.clone());
        }

        Ok(Self {
            providers,
            private_keys,
            rpc_urls,
            wallet_addresses,
        })
    }

    /// Load a wallet from an environment variable
    pub async fn load_wallet_from_env(&mut self, network_name: &str, env_var: &str) -> Result<()> {
        info!(
            "Attempting to load wallet for network {} from env var {}",
            network_name, env_var
        );

        // Check if the network exists
        if !self.providers.contains_key(network_name) {
            return Err(NetworkError::NetworkNotFound(network_name.to_string()).into());
        }

        let private_key = std::env::var(env_var)
            .with_context(|| format!("Environment variable {} not found", env_var))?;

        info!(
            "Successfully read private key from env var {} (length: {})",
            env_var,
            private_key.len()
        );

        let signer = private_key
            .parse::<PrivateKeySigner>()
            .with_context(|| "Failed to parse private key as signer")?;

        // Store the wallet address
        let wallet_address = signer.address();
        self.wallet_addresses
            .insert(network_name.to_string(), wallet_address);

        // Store the private key (we'll create providers with wallets on demand)
        self.private_keys
            .insert(network_name.to_string(), private_key);

        info!(
            "Successfully loaded wallet for network {} with address {}",
            network_name, wallet_address
        );

        Ok(())
    }

    /// Load wallet from key storage
    pub async fn load_wallet_from_key_storage(
        &mut self,
        network_name: &str,
        key_storage: &dyn KeyStorage,
    ) -> Result<()> {
        info!(
            "[PROVIDER DEBUG] Loading wallet from key storage for network {}",
            network_name
        );

        // Check if the network exists
        if !self.providers.contains_key(network_name) {
            error!(
                "[PROVIDER DEBUG] Network '{}' not found in providers map",
                network_name
            );
            error!(
                "[PROVIDER DEBUG] Available networks: {:?}",
                self.providers.keys().collect::<Vec<_>>()
            );
            return Err(NetworkError::NetworkNotFound(network_name.to_string()).into());
        }

        info!(
            "[PROVIDER DEBUG] Network '{}' found, attempting to retrieve key from storage",
            network_name
        );
        let private_key_secret = match key_storage.get_key(network_name).await {
            Ok(key) => {
                info!("[PROVIDER DEBUG] Successfully retrieved key from storage");
                key
            }
            Err(e) => {
                error!(
                    "[PROVIDER DEBUG] Failed to retrieve key from storage: {:?}",
                    e
                );
                return Err(e.context(format!(
                    "Failed to retrieve key for network {}",
                    network_name
                )));
            }
        };

        let private_key = private_key_secret.expose_secret();

        let signer = private_key
            .parse::<PrivateKeySigner>()
            .with_context(|| "Failed to parse private key as signer")?;

        // Store the wallet address
        let wallet_address = signer.address();
        self.wallet_addresses
            .insert(network_name.to_string(), wallet_address);

        // Store the private key (we'll create providers with wallets on demand)
        self.private_keys
            .insert(network_name.to_string(), private_key.to_string());

        info!(
            "Successfully loaded wallet for network {} with address {} from key storage",
            network_name, wallet_address
        );

        Ok(())
    }

    /// Get the chain ID for a given network
    pub async fn get_chain_id(&self, network_name: &str) -> Result<u64> {
        let provider = self.get_provider(network_name)?;
        let chain_id = provider
            .get_chain_id()
            .await
            .with_context(|| format!("Failed to get chain ID for network {}", network_name))?;

        Ok(chain_id)
    }

    /// Get the block number for a given network
    pub async fn get_block_number(&self, network_name: &str) -> Result<u64> {
        let provider = self.get_provider(network_name)?;
        let block_number = provider
            .get_block_number()
            .await
            .with_context(|| format!("Failed to get block number for network {}", network_name))?;

        Ok(block_number)
    }

    /// Get a provider for a given network
    pub fn get_provider(&self, network_name: &str) -> Result<Arc<EthProvider>> {
        self.providers
            .get(network_name)
            .cloned()
            .ok_or_else(|| NetworkError::NetworkNotFound(network_name.to_string()).into())
    }

    /// Get the private key for a network
    pub fn get_private_key(&self, network_name: &str) -> Result<String> {
        self.private_keys.get(network_name).cloned().ok_or_else(|| {
            anyhow::anyhow!(
                "No private key found for network {}. Call load_wallet_from_env first",
                network_name
            )
        })
    }

    /// Get the RPC URL for a network
    pub fn get_rpc_url(&self, network_name: &str) -> Result<&str> {
        self.rpc_urls
            .get(network_name)
            .map(|s| s.as_str())
            .ok_or_else(|| NetworkError::NetworkNotFound(network_name.to_string()).into())
    }

    /// Get a signer for a given network
    #[allow(dead_code)]
    pub fn get_signer(&self, network_name: &str) -> Result<Arc<EthProvider>> {
        // For backward compatibility, check if we have a private key
        if self.private_keys.contains_key(network_name) {
            // Return the regular provider - the actual signing will be handled differently
            self.get_provider(network_name)
        } else {
            Err(anyhow::anyhow!(
                "No signer found for network {}. Call load_wallet_from_env first",
                network_name
            ))
        }
    }

    /// Get all configured network names
    pub fn get_network_names(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    /// Get the wallet address for a given network
    pub fn get_wallet_address(&self, network_name: &str) -> Result<Address> {
        self.wallet_addresses
            .get(network_name)
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No wallet address found for network {}. Call load_wallet_from_env first",
                    network_name
                )
            })
    }

    /// Create a provider from an RPC URL
    async fn create_provider(rpc_url: &str) -> Result<EthProvider> {
        let url =
            Url::parse(rpc_url).with_context(|| format!("Failed to parse RPC URL: {}", rpc_url))?;

        let provider = ProviderBuilder::new().on_http(url);

        // Test connection by getting the current block number
        match provider.get_block_number().await {
            Ok(block_number) => {
                info!(
                    "Connected to RPC at {}, current block: {}",
                    rpc_url, block_number
                );
                Ok(provider)
            }
            Err(err) => {
                error!("Failed to connect to RPC at {}: {}", rpc_url, err);
                Err(NetworkError::ConnectionFailed(err.to_string()).into())
            }
        }
    }
}
