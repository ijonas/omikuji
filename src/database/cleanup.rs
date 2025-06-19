use tokio_cron_scheduler::{Job, JobScheduler};
use chrono::Utc;
use anyhow::{Result, Context};
use tracing::{info, error, debug};
use std::sync::Arc;

use crate::config::models::OmikujiConfig;
use super::repository::FeedLogRepository;

/// Manages the database cleanup task
pub struct CleanupManager {
    config: OmikujiConfig,
    repository: Arc<FeedLogRepository>,
    scheduler: JobScheduler,
}

impl CleanupManager {
    /// Creates a new cleanup manager
    pub async fn new(config: OmikujiConfig, repository: Arc<FeedLogRepository>) -> Result<Self> {
        let scheduler = JobScheduler::new().await
            .context("Failed to create job scheduler")?;
        
        Ok(Self {
            config,
            repository,
            scheduler,
        })
    }

    /// Starts the cleanup scheduler
    pub async fn start(&mut self) -> Result<()> {
        if !self.config.database_cleanup.enabled {
            info!("Database cleanup is disabled in configuration");
            return Ok(());
        }

        let schedule = &self.config.database_cleanup.schedule;
        info!("Starting database cleanup scheduler with cron schedule: {}", schedule);

        // Clone necessary data for the closure
        let config = self.config.clone();
        let repository = Arc::clone(&self.repository);

        // Create the cleanup job
        let job = Job::new_async(schedule.as_str(), move |_uuid, _l| {
            let config = config.clone();
            let repository = Arc::clone(&repository);
            
            Box::pin(async move {
                info!("Running scheduled database cleanup");
                if let Err(e) = run_cleanup(&config, &repository).await {
                    error!("Database cleanup failed: {}", e);
                }
            })
        })
        .context("Failed to create cleanup job")?;

        // Add job to scheduler
        self.scheduler.add(job).await
            .context("Failed to add job to scheduler")?;

        // Start the scheduler
        self.scheduler.start().await
            .context("Failed to start scheduler")?;

        info!("Database cleanup scheduler started successfully");
        Ok(())
    }

    /// Stops the cleanup scheduler
    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping database cleanup scheduler");
        self.scheduler.shutdown().await
            .context("Failed to shutdown scheduler")?;
        Ok(())
    }
}

/// Runs the cleanup task for all datafeeds
async fn run_cleanup(config: &OmikujiConfig, repository: &FeedLogRepository) -> Result<()> {
    let start_time = Utc::now();
    let mut total_deleted = 0u64;
    let mut feed_count = 0;

    // Clean up each datafeed based on its retention configuration
    for datafeed in &config.datafeeds {
        let retention_days = datafeed.data_retention_days;
        
        match repository.delete_older_than(
            &datafeed.name,
            &datafeed.networks,
            retention_days
        ).await {
            Ok(deleted) => {
                if deleted > 0 {
                    debug!(
                        "Deleted {} old records for feed '{}' on network '{}' (retention: {} days)",
                        deleted, datafeed.name, datafeed.networks, retention_days
                    );
                }
                total_deleted += deleted;
                feed_count += 1;
            }
            Err(e) => {
                error!(
                    "Failed to clean up feed '{}' on network '{}': {}",
                    datafeed.name, datafeed.networks, e
                );
            }
        }
    }

    let duration = Utc::now() - start_time;
    
    if total_deleted > 0 {
        info!(
            "Database cleanup completed: deleted {} records across {} feeds in {:.2}s",
            total_deleted, 
            feed_count,
            duration.num_milliseconds() as f64 / 1000.0
        );
    } else {
        debug!(
            "Database cleanup completed: no old records to delete (checked {} feeds in {:.2}s)",
            feed_count,
            duration.num_milliseconds() as f64 / 1000.0
        );
    }

    Ok(())
}

/// Runs a one-time cleanup (useful for testing or manual execution)
pub async fn run_manual_cleanup(
    config: &OmikujiConfig,
    repository: &FeedLogRepository,
) -> Result<()> {
    info!("Running manual database cleanup");
    run_cleanup(config, repository).await
}