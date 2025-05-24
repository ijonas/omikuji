use crate::config::models::Datafeed;
use crate::network::NetworkManager;
use super::fetcher::Fetcher;
use super::json_extractor::JsonExtractor;
use super::contract_updater::ContractUpdater;
use anyhow::Result;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{error, info, debug};

/// Monitors a single datafeed, polling at regular intervals
pub struct FeedMonitor {
    pub(crate) datafeed: Datafeed,
    fetcher: Arc<Fetcher>,
    network_manager: Arc<NetworkManager>,
}

impl FeedMonitor {
    /// Creates a new FeedMonitor for the given datafeed
    pub fn new(datafeed: Datafeed, fetcher: Arc<Fetcher>, network_manager: Arc<NetworkManager>) -> Self {
        Self { datafeed, fetcher, network_manager }
    }
    
    /// Starts monitoring the datafeed
    /// This runs indefinitely, polling at the configured interval
    pub async fn start(self) {
        let mut interval = interval(Duration::from_secs(self.datafeed.check_frequency));
        
        info!(
            "Starting feed monitor for '{}' with {}s interval",
            self.datafeed.name, self.datafeed.check_frequency
        );
        
        loop {
            interval.tick().await;
            
            match self.poll_once().await {
                Ok((value, timestamp)) => {
                    info!(
                        "Datafeed {}: value={}, timestamp={}",
                        self.datafeed.name, value, timestamp
                    );
                    
                    // Check if contract update is needed based on time
                    if let Err(e) = self.check_and_update_contract(value).await {
                        error!(
                            "Failed to update contract for datafeed {}: {}",
                            self.datafeed.name, e
                        );
                    }
                }
                Err(e) => {
                    error!(
                        "Datafeed {}: {}",
                        self.datafeed.name, e
                    );
                }
            }
        }
    }
    
    /// Performs a single poll of the datafeed
    /// Returns (value, timestamp) on success
    async fn poll_once(&self) -> Result<(f64, u64)> {
        // Fetch JSON from the feed URL
        let json = self.fetcher
            .fetch_json(&self.datafeed.feed_url)
            .await?;
        
        // Extract value and timestamp
        let (value, timestamp) = JsonExtractor::extract_feed_data(
            &json,
            &self.datafeed.feed_json_path,
            self.datafeed.feed_json_path_timestamp.as_deref(),
        )?;
        
        Ok((value, timestamp))
    }
    
    /// Checks if contract update is needed and submits if necessary
    async fn check_and_update_contract(&self, value: f64) -> Result<()> {
        let updater = ContractUpdater::new(&self.network_manager);
        
        // Check if update is needed based on time
        match updater.should_update_based_on_time(&self.datafeed).await {
            Ok(true) => {
                info!(
                    "Time-based update triggered for datafeed {}",
                    self.datafeed.name
                );
                
                // Submit the value to the contract
                updater.submit_value(&self.datafeed, value).await?;
                Ok(())
            }
            Ok(false) => {
                debug!(
                    "No update needed for datafeed {} - minimum frequency not reached",
                    self.datafeed.name
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    "Failed to check update requirement for datafeed {}: {}",
                    self.datafeed.name, e
                );
                Err(e)
            }
        }
    }
}