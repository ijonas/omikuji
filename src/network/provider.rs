use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use alloy::{
    network::EthereumWallet,
    providers::{Provider, ProviderBuilder, RootProvider},
    signers::local::PrivateKeySigner,
    transports::http::{Client, Http},
};
use thiserror::Error;
use tracing::{debug, error, info, warn};
use url::Url;

use crate::config::models::Network;

/// Errors that can occur when interacting with network providers
#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("Network not found: {0}")]
    NetworkNotFound(String),

    #[error("Provider error: {0}")]
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
    
    /// Providers with signers for each network
    signers: HashMap<String, Arc<EthProvider>>,
    
    /// RPC URLs for each network (needed for creating signed providers)
    rpc_urls: HashMap<String, String>,
}

impl NetworkManager {
    /// Create a new network manager from a list of network configurations
    pub async fn new(networks: &[Network]) -> Result<Self> {
        let mut providers = HashMap::new();
        let signers = HashMap::new();
        let mut rpc_urls = HashMap::new();

        for network in networks {
            let provider = Self::create_provider(&network.rpc_url)
                .await
                .with_context(|| format!("Failed to create provider for network {}", network.name))?;
            
            providers.insert(network.name.clone(), Arc::new(provider));
            rpc_urls.insert(network.name.clone(), network.rpc_url.clone());
        }

        Ok(Self {
            providers,
            signers,
            rpc_urls,
        })
    }

    /// Load a wallet from an environment variable
    pub async fn load_wallet_from_env(&mut self, network_name: &str, env_var: &str) -> Result<()> {
        let private_key = std::env::var(env_var)
            .with_context(|| format!("Environment variable {} not found", env_var))?;

        let signer = private_key
            .parse::<PrivateKeySigner>()
            .with_context(|| "Failed to parse private key as signer")?;

        let _wallet = EthereumWallet::from(signer);

        debug!("Loaded wallet for network {}", network_name);

        // Get the RPC URL for this network
        let rpc_url = self.rpc_urls
            .get(network_name)
            .ok_or_else(|| NetworkError::NetworkNotFound(network_name.to_string()))?;
        
        let _url = Url::parse(rpc_url)
            .with_context(|| format!("Failed to parse RPC URL: {}", rpc_url))?;
        
        // For now, we'll store the wallet separately and handle signing differently
        // This is a temporary solution until we figure out the proper type handling
        // TODO: Implement proper wallet integration with alloy
        
        warn!("Wallet loading not fully implemented for alloy migration");
        
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

    /// Get a signer for a given network
    pub fn get_signer(&self, network_name: &str) -> Result<Arc<EthProvider>> {
        self.signers
            .get(network_name)
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!("No signer found for network {}. Call load_wallet_from_env first", network_name)
            })
    }

    /// Get all configured network names
    pub fn get_network_names(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    /// Create a provider from an RPC URL
    async fn create_provider(rpc_url: &str) -> Result<EthProvider> {
        let url = Url::parse(rpc_url)
            .with_context(|| format!("Failed to parse RPC URL: {}", rpc_url))?;

        let provider = ProviderBuilder::new()
            .on_http(url);

        // Test connection by getting the current block number
        match provider.get_block_number().await {
            Ok(block_number) => {
                info!("Connected to RPC at {}, current block: {}", rpc_url, block_number);
                Ok(provider)
            }
            Err(err) => {
                error!("Failed to connect to RPC at {}: {}", rpc_url, err);
                Err(NetworkError::ConnectionFailed(err.to_string()).into())
            }
        }
    }
}