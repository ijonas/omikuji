// src/tui/db_metrics.rs
// Periodically update dashboard metrics from the database (transaction stats)

use std::sync::Arc;
use tokio::sync::RwLock;
use crate::database::TransactionLogRepository;
use crate::tui::DashboardState;
use crate::tui::metrics::update_metrics_from_stats;

/// Spawns a background task to periodically update dashboard metrics from the DB
pub fn spawn_db_metrics_updater(
    dashboard: Arc<RwLock<DashboardState>>,
    tx_log_repo: Arc<TransactionLogRepository>,
    interval_secs: u64,
) {
    tokio::spawn(async move {
        loop {
            if let Ok(stats) = tx_log_repo.get_stats().await {
                update_metrics_from_stats(&dashboard, &stats).await;
            }
            tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;
        }
    });
}
