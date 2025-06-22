use crate::gas_price::GasPriceManager;
use crate::metrics::{FeedMetrics, EconomicMetrics};
use crate::network::NetworkManager;
use alloy::primitives::utils::format_units;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info};

/// Monitors wallet balances across all networks
pub struct WalletBalanceMonitor {
    network_manager: Arc<NetworkManager>,
    gas_price_manager: Option<Arc<GasPriceManager>>,
    update_interval_seconds: u64,
    /// Track daily spending for runway calculation (network -> daily spend in USD)
    daily_spending_estimates: HashMap<String, f64>,
}

impl WalletBalanceMonitor {
    /// Create a new wallet balance monitor
    pub fn new(network_manager: Arc<NetworkManager>) -> Self {
        Self {
            network_manager,
            gas_price_manager: None,
            update_interval_seconds: 60, // Default to 1 minute
            daily_spending_estimates: HashMap::new(),
        }
    }
    
    /// Set the gas price manager
    pub fn with_gas_price_manager(mut self, gas_price_manager: Arc<GasPriceManager>) -> Self {
        self.gas_price_manager = Some(gas_price_manager);
        self
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
    async fn update_network_balance(
        &self,
        network_name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Get the wallet address for this network
        let address = self.network_manager.get_wallet_address(network_name)?;

        // Get the provider to query balance
        let provider = self.network_manager.get_provider(network_name)?;

        // Fetch the balance
        use alloy::providers::Provider;
        match provider.get_balance(address).await {
            Ok(balance) => {
                let balance_wei = balance.to::<u128>();
                let balance_native = balance_wei as f64 / 1e18; // Convert to native units

                // Update basic balance metric
                FeedMetrics::set_wallet_balance(
                    network_name,
                    &format!("{:?}", address),
                    balance_wei,
                );

                // Get native token price from gas price manager if available
                let native_token_price = if let Some(ref gas_price_manager) = self.gas_price_manager {
                    if let Some(price_info) = gas_price_manager.get_price(network_name).await {
                        price_info.price_usd
                    } else {
                        1.0 // Default if price not available
                    }
                } else {
                    1.0 // Default price if no gas price manager
                };
                
                // Update economic metrics
                EconomicMetrics::update_wallet_balance_usd(
                    network_name,
                    &format!("{:?}", address),
                    balance_native,
                    native_token_price,
                );
                
                // Update runway if we have spending data
                if let Some(&daily_spend) = self.daily_spending_estimates.get(network_name) {
                    let balance_usd = balance_native * native_token_price;
                    EconomicMetrics::update_runway_days(
                        network_name,
                        &format!("{:?}", address),
                        balance_usd,
                        daily_spend,
                    );
                    
                    EconomicMetrics::update_daily_spending_rate(network_name, daily_spend);
                }

                debug!(
                    "Updated wallet balance for {} on {}: {} wei ({} ETH, ${:.2} USD)",
                    address,
                    network_name,
                    balance_wei,
                    format_units(balance, "ether").unwrap_or_else(|_| "error".to_string()),
                    balance_native * native_token_price
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
