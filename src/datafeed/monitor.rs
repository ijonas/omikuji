use crate::config::models::Datafeed;
use super::fetcher::Fetcher;
use super::json_extractor::JsonExtractor;
use anyhow::Result;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{error, info};

/// Monitors a single datafeed, polling at regular intervals
pub struct FeedMonitor {
    pub(crate) datafeed: Datafeed,
    fetcher: Arc<Fetcher>,
}

impl FeedMonitor {
    /// Creates a new FeedMonitor for the given datafeed
    pub fn new(datafeed: Datafeed, fetcher: Arc<Fetcher>) -> Self {
        Self { datafeed, fetcher }
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
}