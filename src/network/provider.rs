use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use ethers::prelude::*;
use thiserror::Error;
use tracing::{debug, error, info};
use url::Url;

use crate::config::models::Network;

/// Errors that can occur when interacting with network providers
#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("Network not found: {0}")]
    NetworkNotFound(String),

    #[error("Provider error: {0}")]
    ProviderError(#[from] ProviderError),

    #[error("RPC connection failed: {0}")]
    ConnectionFailed(String),
}

/// Type alias for the ethers-rs provider we will use
pub type EthProvider = Provider<Http>;

/// Manages the connections to different EVM networks
#[derive(Debug)]
pub struct NetworkManager {
    /// Map of network name to provider
    providers: HashMap<String, Arc<EthProvider>>,
    
    /// Signers for each network, keyed by network name
    signers: HashMap<String, Arc<SignerMiddleware<Arc<EthProvider>, LocalWallet>>>,
}

impl NetworkManager {
    /// Create a new network manager from a list of network configurations
    pub async fn new(networks: &[Network]) -> Result<Self> {
        let mut providers = HashMap::new();
        let signers = HashMap::new();

        for network in networks {
            let provider = Self::create_provider(&network.rpc_url)
                .await
                .with_context(|| format!("Failed to create provider for network {}", network.name))?;
            
            providers.insert(network.name.clone(), Arc::new(provider));
        }

        Ok(Self {
            providers,
            signers,
        })
    }

    /// Load a wallet from an environment variable
    pub async fn load_wallet_from_env(&mut self, network_name: &str, env_var: &str) -> Result<()> {
        let provider = self.get_provider(network_name)?;

        let private_key = std::env::var(env_var)
            .with_context(|| format!("Environment variable {} not found", env_var))?;

        let wallet = private_key
            .parse::<LocalWallet>()
            .with_context(|| "Failed to parse private key as wallet")?;

        let chain_id = self.get_chain_id(network_name).await?;
        let wallet = wallet.with_chain_id(chain_id);

        debug!("Loaded wallet for network {}", network_name);

        let signer = SignerMiddleware::new(provider.clone(), wallet);
        self.signers.insert(network_name.to_string(), Arc::new(signer));

        Ok(())
    }

    /// Get the chain ID for a given network
    pub async fn get_chain_id(&self, network_name: &str) -> Result<u64> {
        let provider = self.get_provider(network_name)?;
        let chain_id = provider
            .get_chainid()
            .await
            .with_context(|| format!("Failed to get chain ID for network {}", network_name))?;
        
        Ok(chain_id.as_u64())
    }

    /// Get the block number for a given network
    pub async fn get_block_number(&self, network_name: &str) -> Result<u64> {
        let provider = self.get_provider(network_name)?;
        let block_number = provider
            .get_block_number()
            .await
            .with_context(|| format!("Failed to get block number for network {}", network_name))?;
        
        Ok(block_number.as_u64())
    }

    /// Get a provider for a given network
    pub fn get_provider(&self, network_name: &str) -> Result<Arc<EthProvider>> {
        self.providers
            .get(network_name)
            .cloned()
            .ok_or_else(|| NetworkError::NetworkNotFound(network_name.to_string()).into())
    }

    /// Get a signer for a given network
    pub fn get_signer(&self, network_name: &str) -> Result<Arc<SignerMiddleware<Arc<EthProvider>, LocalWallet>>> {
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

        let http_provider = Http::new(url);
        let mut provider = Provider::new(http_provider);

        // Configure timeouts for the provider
        provider.set_interval(Duration::from_millis(2000));

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