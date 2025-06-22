// src/tui/metrics.rs
// Helper for updating dashboard metrics from TransactionStats

use crate::database::transaction_repository::TransactionStats;
use crate::tui::DashboardState;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Update dashboard metrics from a list of TransactionStats (aggregated from DB)
pub async fn update_metrics_from_stats(dashboard: &Arc<RwLock<DashboardState>>, stats: &[TransactionStats]) {
    let mut dash = dashboard.write().await;
    let mut total_txs = 0;
    let mut last_tx_cost = None;
    let mut total_cost_eth = 0.0;
    let mut cost_count = 0;
    for stat in stats {
        total_txs += stat.total_transactions as usize;
        if let Some(cost_wei) = &stat.total_cost_wei {
            if let Ok(cost_wei) = cost_wei.parse::<f64>() {
                total_cost_eth += cost_wei / 1e18;
                cost_count += 1;
                last_tx_cost = Some(cost_wei / 1e18);
            }
        }
    }
    dash.metrics.tx_count = total_txs;
    dash.metrics.last_tx_cost = last_tx_cost;
    dash.metrics.avg_tx_cost = if cost_count > 0 { Some(total_cost_eth / cost_count as f64) } else { None };
}
