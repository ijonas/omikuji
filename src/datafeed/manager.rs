use crate::config::models::{OmikujiConfig, Datafeed};
use crate::network::NetworkManager;
use super::fetcher::Fetcher;
use super::monitor::FeedMonitor;
use super::contract_config::ContractConfigReader;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{info, error};

/// Manages all datafeed monitors
pub struct FeedManager {
    config: OmikujiConfig,
    network_manager: Arc<NetworkManager>,
    fetcher: Arc<Fetcher>,
    handles: Vec<JoinHandle<()>>,
}

impl FeedManager {
    /// Creates a new FeedManager with the given configuration
    pub fn new(config: OmikujiConfig, network_manager: Arc<NetworkManager>) -> Self {
        Self {
            config,
            network_manager,
            fetcher: Arc::new(Fetcher::new()),
            handles: Vec::new(),
        }
    }
    
    /// Starts monitoring all configured datafeeds
    /// Each datafeed runs in its own tokio task
    pub async fn start(&mut self) {
        info!("Starting feed manager with {} datafeeds", self.config.datafeeds.len());
        
        let contract_reader = ContractConfigReader::new(&self.network_manager);
        
        for mut datafeed in self.config.datafeeds.clone() {
            // Configure the datafeed (either from contract or YAML)
            match self.configure_datafeed(&mut datafeed, &contract_reader).await {
                Ok(_) => {
                    let handle = self.spawn_monitor(datafeed);
                    self.handles.push(handle);
                }
                Err(feed_name) => {
                    // Feed was skipped due to error - already logged
                    info!("Skipping datafeed '{}'", feed_name);
                }
            }
        }
        
        info!("Feed manager initialization complete. {} feeds running.", self.handles.len());
    }
    
    /// Configures a datafeed by reading from contract or using YAML values
    /// Returns Ok(()) if successful, Err(feed_name) if the feed should be skipped
    async fn configure_datafeed(
        &self,
        datafeed: &mut Datafeed,
        contract_reader: &ContractConfigReader<'_>,
    ) -> Result<(), String> {
        if datafeed.read_contract_config {
            // Try to read from contract
            match contract_reader.read_config(&datafeed.networks, &datafeed.contract_address).await {
                Ok(contract_config) => {
                    self.apply_contract_config(datafeed, contract_config);
                    Ok(())
                }
                Err(e) => {
                    error!(
                        "Failed to read contract config for datafeed '{}': {}. Skipping this feed.",
                        datafeed.name, e
                    );
                    Err(datafeed.name.clone())
                }
            }
        } else {
            // Use YAML config
            self.log_datafeed_config(datafeed, "config.yaml values");
            Ok(())
        }
    }
    
    /// Applies contract configuration to a datafeed
    fn apply_contract_config(&self, datafeed: &mut Datafeed, config: crate::datafeed::contract_config::ContractConfig) {
        self.log_config_values(
            &datafeed.name,
            "contract config",
            config.decimals,
            config.min_value,
            config.max_value,
        );
        
        datafeed.decimals = Some(config.decimals);
        datafeed.min_value = Some(config.min_value);
        datafeed.max_value = Some(config.max_value);
    }
    
    /// Logs datafeed configuration from YAML
    fn log_datafeed_config(&self, datafeed: &Datafeed, source: &str) {
        self.log_config_values(
            &datafeed.name,
            source,
            datafeed.decimals.unwrap_or(0),
            datafeed.min_value.unwrap_or(0),
            datafeed.max_value.unwrap_or(0),
        );
    }
    
    /// Common logging for configuration values
    fn log_config_values(
        &self,
        name: &str,
        source: &str,
        decimals: impl std::fmt::Display,
        min_value: impl std::fmt::Display,
        max_value: impl std::fmt::Display,
    ) {
        info!(
            "Using {} for datafeed '{}': decimals={}, min_value={}, max_value={}",
            source, name, decimals, min_value, max_value
        );
    }
    
    /// Spawns a monitor task for a single datafeed
    fn spawn_monitor(&self, datafeed: Datafeed) -> JoinHandle<()> {
        let monitor = FeedMonitor::new(
            datafeed.clone(), 
            Arc::clone(&self.fetcher),
            Arc::clone(&self.network_manager),
            self.config.clone()
        );
        let feed_name = datafeed.name.clone();
        
        tokio::spawn(async move {
            info!("Spawning monitor task for datafeed '{}'", feed_name);
            monitor.start().await;
        })
    }
    
    /// Waits for all monitors to complete (they run indefinitely)
    #[allow(dead_code)]
    pub async fn wait(&mut self) {
        for handle in self.handles.drain(..) {
            if let Err(e) = handle.await {
                error!("Monitor task panicked: {}", e);
            }
        }
    }
}