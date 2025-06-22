// src/tui/update.rs
// Helper functions for updating the TUI dashboard state from the main app logic.
// Use these functions to push new metrics and history values into the dashboard.

use std::sync::Arc;
use tokio::sync::RwLock;
use crate::tui::{DashboardState, FeedStatus};

/// Push a new gas price (in Gwei) into the dashboard's history buffer.
pub async fn push_gas_price(dashboard: &Arc<RwLock<DashboardState>>, value: f64) {
    let mut dash = dashboard.write().await;
    dash.gas_price_gwei_hist.push(value);
}

/// Push a new response time (in ms) into the dashboard's history buffer.
pub async fn push_response_time(dashboard: &Arc<RwLock<DashboardState>>, value: u64) {
    let mut dash = dashboard.write().await;
    dash.response_time_ms_hist.push(value);
}

/// Record the result of a feed update (success/failure).
pub async fn record_update_result(dashboard: &Arc<RwLock<DashboardState>>, success: bool) {
    let mut dash = dashboard.write().await;
    dash.metrics.update_total_count += 1;
    if success {
        dash.metrics.update_success_count += 1;
    }
}

/// Update the average staleness (in seconds) for all feeds.
pub async fn update_avg_staleness(dashboard: &Arc<RwLock<DashboardState>>) {
    let mut dash = dashboard.write().await;
    let sum: u64 = dash.feeds.iter().map(|f| f.last_update.elapsed().as_secs()).sum();
    let count = dash.feeds.len() as u64;
    dash.metrics.avg_staleness_secs = if count > 0 { sum as f64 / count as f64 } else { 0.0 };
}

/// Update feed, error, and tx counts (call after feeds or txs change).
pub async fn update_counts(dashboard: &Arc<RwLock<DashboardState>>) {
    let mut dash = dashboard.write().await;
    dash.metrics.feed_count = dash.feeds.len();
    dash.metrics.error_count = dash.feeds.iter().filter(|f| f.error.is_some()).count();
    // Optionally update tx_count from your tx logic
}

/// Set the last tx cost (in ETH).
pub async fn set_last_tx_cost(dashboard: &Arc<RwLock<DashboardState>>, cost: f64) {
    let mut dash = dashboard.write().await;
    dash.metrics.last_tx_cost = Some(cost);
}

// Add more helpers as needed for your app's metrics.
