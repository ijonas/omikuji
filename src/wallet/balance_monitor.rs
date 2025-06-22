use crate::network::NetworkManager;
use crate::metrics::FeedMetrics;
use alloy::primitives::utils::format_units;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{info, error, debug};

/// Monitors wallet balances across all networks
pub struct WalletBalanceMonitor {
    network_manager: Arc<NetworkManager>,
    update_interval_seconds: u64,
}

impl WalletBalanceMonitor {
    /// Create a new wallet balance monitor
    pub fn new(network_manager: Arc<NetworkManager>) -> Self {
        Self {
            network_manager,
            update_interval_seconds: 60, // Default to 1 minute
        }
    }

    /// Start monitoring wallet balances
    pub async fn start(self) {
        let mut interval = interval(Duration::from_secs(self.update_interval_seconds));
        
        info!(
            "Starting wallet balance monitor with {}s interval",
            self.update_interval_seconds
        );

        loop {
            interval.tick().await;
            self.update_all_balances().await;
        }
    }

    /// Update balances for all networks
    async fn update_all_balances(&self) {
        // Get all network names from the network manager
        let networks = self.network_manager.get_network_names();
        
        for network_name in networks {
            if let Err(e) = self.update_network_balance(&network_name).await {
                error!(
                    "Failed to update wallet balance for network {}: {}",
                    network_name, e
                );
            }
        }
    }

    /// Update balance for a specific network
    async fn update_network_balance(&self, network_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Get the wallet address for this network
        let address = self.network_manager.get_wallet_address(network_name)?;
        
        // Get the provider to query balance
        let provider = self.network_manager.get_provider(network_name)?;
        
        // Fetch the balance  
        use alloy::providers::Provider;
        match provider.get_balance(address).await {
            Ok(balance) => {
                let balance_wei = balance.to::<u128>();
                
                // Update Prometheus metric
                FeedMetrics::set_wallet_balance(
                    network_name,
                    &format!("{:?}", address),
                    balance_wei,
                );
                
                debug!(
                    "Updated wallet balance for {} on {}: {} wei ({} ETH)",
                    address,
                    network_name,
                    balance_wei,
                    format_units(balance, "ether").unwrap_or_else(|_| "error".to_string())
                );
                
                Ok(())
            }
            Err(e) => {
                error!(
                    "Failed to fetch balance for {} on {}: {}",
                    address, network_name, e
                );
                Err(Box::new(e))
            }
        }
    }
}