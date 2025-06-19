use crate::config::models::{Datafeed, OmikujiConfig};
use crate::network::NetworkManager;
use crate::database::{FeedLogRepository, TransactionLogRepository};
use crate::database::models::NewFeedLog;
use crate::metrics::FeedMetrics;
use super::fetcher::Fetcher;
use super::json_extractor::JsonExtractor;
use super::contract_updater::ContractUpdater;
use anyhow::Result;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{error, info, debug, warn};

/// Monitors a single datafeed, polling at regular intervals
pub struct FeedMonitor {
    pub(crate) datafeed: Datafeed,
    fetcher: Arc<Fetcher>,
    network_manager: Arc<NetworkManager>,
    config: OmikujiConfig,
    repository: Option<Arc<FeedLogRepository>>,
    tx_log_repo: Option<Arc<TransactionLogRepository>>,
}

impl FeedMonitor {
    /// Creates a new FeedMonitor for the given datafeed
    pub fn new(
        datafeed: Datafeed, 
        fetcher: Arc<Fetcher>, 
        network_manager: Arc<NetworkManager>, 
        config: OmikujiConfig,
        repository: Option<Arc<FeedLogRepository>>,
        tx_log_repo: Option<Arc<TransactionLogRepository>>,
    ) -> Self {
        Self { datafeed, fetcher, network_manager, config, repository, tx_log_repo }
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
                    
                    // Update Prometheus metrics
                    FeedMetrics::set_feed_value(
                        &self.datafeed.name,
                        &self.datafeed.networks,
                        value,
                        timestamp,
                    );
                    
                    // Update contract metrics (read current contract state)
                    let updater = if let Some(ref tx_repo) = self.tx_log_repo {
                        ContractUpdater::with_tx_logging(&self.network_manager, &self.config, tx_repo.clone())
                    } else {
                        ContractUpdater::new(&self.network_manager, &self.config)
                    };
                    
                    if let Err(e) = updater.update_contract_metrics(&self.datafeed, value).await {
                        error!(
                            "Failed to update contract metrics for {}: {}",
                            self.datafeed.name, e
                        );
                    }
                    
                    // Save to database if repository is available
                    if let Some(ref repository) = self.repository {
                        debug!(
                            "Saving feed log to database for {}: value={}, timestamp={}",
                            self.datafeed.name, value, timestamp
                        );
                        
                        let log = NewFeedLog {
                            feed_name: self.datafeed.name.clone(),
                            network_name: self.datafeed.networks.clone(),
                            feed_value: value,
                            feed_timestamp: timestamp as i64,
                            error_status_code: None,
                            network_error: false,
                        };
                        
                        match repository.save(log).await {
                            Ok(saved_log) => {
                                debug!(
                                    "Feed log saved successfully for {} with id={}",
                                    self.datafeed.name, saved_log.id
                                );
                            }
                            Err(e) => {
                                error!(
                                    "Failed to save feed log for {}: {}",
                                    self.datafeed.name, e
                                );
                            }
                        }
                    } else {
                        debug!(
                            "No database repository configured for feed {}, skipping database save",
                            self.datafeed.name
                        );
                    }
                    
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
                    
                    // Save error to database if repository is available
                    if let Some(ref repository) = self.repository {
                        self.save_error_log(repository, &e).await;
                    }
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
        let updater = if let Some(ref tx_repo) = self.tx_log_repo {
            ContractUpdater::with_tx_logging(&self.network_manager, &self.config, tx_repo.clone())
        } else {
            ContractUpdater::new(&self.network_manager, &self.config)
        };
        
        // Check if update is needed
        let (should_update, reason) = updater.check_update_needed(&self.datafeed, value).await?;
        
        if should_update {
            info!(
                "Update triggered for datafeed {} due to {}",
                self.datafeed.name, reason
            );
            
            // Submit the value to the contract
            updater.submit_value(&self.datafeed, value).await?;
        } else {
            debug!(
                "No update needed for datafeed {} - neither time nor deviation thresholds met",
                self.datafeed.name
            );
        }
        
        Ok(())
    }
    
    /// Saves an error log entry to the database
    async fn save_error_log(&self, repository: &FeedLogRepository, error: &anyhow::Error) {
        // Try to determine if it's an HTTP error or network error
        let (error_status_code, network_error) = if let Some(http_err) = error.downcast_ref::<super::fetcher::FetchError>() {
            match http_err {
                super::fetcher::FetchError::HttpError(status_code) => (Some(*status_code as i32), false),
                _ => (None, true),
            }
        } else {
            (None, true)
        };
        
        debug!(
            "Saving error log for feed {}: error_status={:?}, network_error={}, error_message={}",
            self.datafeed.name, error_status_code, network_error, error
        );
        
        let log = NewFeedLog {
            feed_name: self.datafeed.name.clone(),
            network_name: self.datafeed.networks.clone(),
            feed_value: 0.0, // Default value for errors
            feed_timestamp: chrono::Utc::now().timestamp(),
            error_status_code,
            network_error,
        };
        
        match repository.save(log).await {
            Ok(saved_log) => {
                debug!(
                    "Error log saved successfully for {} with id={}",
                    self.datafeed.name, saved_log.id
                );
            }
            Err(e) => {
                warn!(
                    "Failed to save error log for feed {}: {}",
                    self.datafeed.name, e
                );
            }
        }
    }
}