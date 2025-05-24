use crate::config::models::{OmikujiConfig, Datafeed};
use super::fetcher::Fetcher;
use super::monitor::FeedMonitor;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{info, error};

/// Manages all datafeed monitors
pub struct FeedManager {
    config: OmikujiConfig,
    fetcher: Arc<Fetcher>,
    handles: Vec<JoinHandle<()>>,
}

impl FeedManager {
    /// Creates a new FeedManager with the given configuration
    pub fn new(config: OmikujiConfig) -> Self {
        Self {
            config,
            fetcher: Arc::new(Fetcher::new()),
            handles: Vec::new(),
        }
    }
    
    /// Starts monitoring all configured datafeeds
    /// Each datafeed runs in its own tokio task
    pub async fn start(&mut self) {
        info!("Starting feed manager with {} datafeeds", self.config.datafeeds.len());
        
        for datafeed in self.config.datafeeds.clone() {
            let handle = self.spawn_monitor(datafeed);
            self.handles.push(handle);
        }
        
        info!("All datafeed monitors started");
    }
    
    /// Spawns a monitor task for a single datafeed
    fn spawn_monitor(&self, datafeed: Datafeed) -> JoinHandle<()> {
        let monitor = FeedMonitor::new(datafeed.clone(), Arc::clone(&self.fetcher));
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