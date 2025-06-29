use crate::gas_price::GasPriceManager;
use crate::metrics::{EconomicMetrics, FeedMetrics};
use crate::network::NetworkManager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

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
                FeedMetrics::set_wallet_balance(network_name, &format!("{address:?}"), balance_wei);

                // Get native token price from gas price manager if available
                let native_token_price = if let Some(ref gas_price_manager) = self.gas_price_manager
                {
                    if let Some(price_info) = gas_price_manager.get_price(network_name).await {
                        debug!(
                            "Got price for {} from gas price manager: ${:.2} USD (token: {})",
                            network_name, price_info.price_usd, price_info.symbol
                        );
                        price_info.price_usd
                    } else {
                        warn!(
                            "No price available for {} from gas price manager, using default $1.0",
                            network_name
                        );
                        1.0 // Default if price not available
                    }
                } else {
                    debug!("No gas price manager configured, using default price $1.0");
                    1.0 // Default price if no gas price manager
                };

                // Update economic metrics
                EconomicMetrics::update_wallet_balance_usd(
                    network_name,
                    &format!("{address:?}"),
                    balance_native,
                    native_token_price,
                );

                // Update runway if we have spending data
                if let Some(&daily_spend) = self.daily_spending_estimates.get(network_name) {
                    let balance_usd = balance_native * native_token_price;
                    EconomicMetrics::update_runway_days(
                        network_name,
                        &format!("{address:?}"),
                        balance_usd,
                        daily_spend,
                    );

                    EconomicMetrics::update_daily_spending_rate(network_name, daily_spend);
                }

                debug!(
                    "Updated wallet balance for {} on {}: {} wei ({:.6} native tokens, ${:.2} USD @ ${:.2}/token)",
                    address,
                    network_name,
                    balance_wei,
                    balance_native,
                    balance_native * native_token_price,
                    native_token_price
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gas_price::models::{GasPriceFeedConfig, CoinGeckoConfig};

    #[test]
    fn test_wallet_balance_monitor_creation() {
        // Create a minimal network manager for testing
        let networks = vec![];
        let network_manager = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(NetworkManager::new(&networks))
            .unwrap();
        let network_manager = Arc::new(network_manager);
        
        let monitor = WalletBalanceMonitor::new(network_manager.clone());
        
        assert_eq!(monitor.update_interval_seconds, 60);
        assert!(monitor.gas_price_manager.is_none());
        assert!(monitor.daily_spending_estimates.is_empty());
    }

    #[test]
    fn test_wallet_balance_monitor_with_gas_price_manager() {
        let networks = vec![];
        let network_manager = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(NetworkManager::new(&networks))
            .unwrap();
        let network_manager = Arc::new(network_manager);
        
        let gas_config = GasPriceFeedConfig {
            enabled: true,
            update_frequency: 60,
            provider: "coingecko".to_string(),
            coingecko: CoinGeckoConfig {
                api_key: None,
                base_url: "https://api.coingecko.com/api/v3".to_string(),
            },
            fallback_to_cache: true,
            persist_to_database: false,
        };
        let gas_price_manager = Arc::new(GasPriceManager::new(
            gas_config,
            HashMap::new(),
            None,
        ));
        
        let monitor = WalletBalanceMonitor::new(network_manager)
            .with_gas_price_manager(gas_price_manager.clone());
        
        assert!(monitor.gas_price_manager.is_some());
    }

    #[test]
    fn test_daily_spending_estimates() {
        let networks = vec![];
        let network_manager = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(NetworkManager::new(&networks))
            .unwrap();
        let network_manager = Arc::new(network_manager);
        
        let mut monitor = WalletBalanceMonitor::new(network_manager);
        
        // Test adding spending estimates
        monitor.daily_spending_estimates.insert("network1".to_string(), 10.5);
        monitor.daily_spending_estimates.insert("network2".to_string(), 25.0);
        
        assert_eq!(monitor.daily_spending_estimates.get("network1"), Some(&10.5));
        assert_eq!(monitor.daily_spending_estimates.get("network2"), Some(&25.0));
        assert_eq!(monitor.daily_spending_estimates.get("network3"), None);
    }

    #[test]
    fn test_update_interval() {
        let networks = vec![];
        let network_manager = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(NetworkManager::new(&networks))
            .unwrap();
        let network_manager = Arc::new(network_manager);
        
        let monitor = WalletBalanceMonitor::new(network_manager);
        assert_eq!(monitor.update_interval_seconds, 60);
    }

    #[tokio::test]
    async fn test_update_network_balance_error_handling() {
        let networks = vec![];
        let network_manager = NetworkManager::new(&networks).await.unwrap();
        let network_manager = Arc::new(network_manager);
        
        let monitor = WalletBalanceMonitor::new(network_manager.clone());
        
        // Test balance update with non-existent network
        let result = monitor.update_network_balance("non-existent-network").await;
        assert!(result.is_err());
        
        // Test error message contains expected text
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("Network not found") || 
                error_message.contains("No wallet address found"));
    }

    #[test]
    fn test_monitor_fields() {
        let networks = vec![];
        let network_manager = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(NetworkManager::new(&networks))
            .unwrap();
        let network_manager = Arc::new(network_manager);
        
        let gas_config = GasPriceFeedConfig {
            enabled: true,
            update_frequency: 30,
            provider: "coingecko".to_string(),
            coingecko: CoinGeckoConfig {
                api_key: None,
                base_url: "https://api.coingecko.com/api/v3".to_string(),
            },
            fallback_to_cache: true,
            persist_to_database: false,
        };
        let gas_price_manager = Arc::new(GasPriceManager::new(
            gas_config,
            HashMap::new(),
            None,
        ));
        
        let mut monitor = WalletBalanceMonitor::new(network_manager.clone());
        
        // Test initial state
        assert_eq!(monitor.update_interval_seconds, 60);
        assert!(monitor.gas_price_manager.is_none());
        assert!(monitor.daily_spending_estimates.is_empty());
        
        // Test with gas price manager
        monitor = monitor.with_gas_price_manager(gas_price_manager);
        assert!(monitor.gas_price_manager.is_some());
        
        // Test daily spending estimates
        monitor.daily_spending_estimates.insert("eth-mainnet".to_string(), 50.0);
        assert_eq!(monitor.daily_spending_estimates.len(), 1);
        assert_eq!(monitor.daily_spending_estimates.get("eth-mainnet"), Some(&50.0));
    }

    #[test]
    fn test_balance_conversion() {
        // Test wei to native conversion
        let balance_wei = 1_500_000_000_000_000_000u128; // 1.5 ETH in wei
        let balance_native = balance_wei as f64 / 1e18;
        assert!((balance_native - 1.5).abs() < 0.0000001);
        
        // Test balance in USD calculation
        let price_usd = 2000.0;
        let balance_usd = balance_native * price_usd;
        assert!((balance_usd - 3000.0).abs() < 0.01);
        
        // Test runway calculation
        let daily_spend = 100.0;
        let runway_days = balance_usd / daily_spend;
        assert!((runway_days - 30.0).abs() < 0.01);
    }
}
