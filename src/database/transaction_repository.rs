use super::connection::DatabasePool;
use crate::metrics::gas_metrics::TransactionDetails;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use tracing::debug;

/// Repository for transaction log operations
pub struct TransactionLogRepository {
    pool: DatabasePool,
}

/// Transaction log entry from database
#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct TransactionLog {
    pub id: i32,
    pub tx_hash: String,
    pub feed_name: String,
    pub network_name: String,
    pub gas_limit: i64,
    pub gas_used: i64,
    pub gas_price_gwei: f64,
    pub total_cost_wei: String, // Store as string to avoid BigDecimal issues
    pub efficiency_percent: f64,
    pub tx_type: String,
    pub status: String,
    pub block_number: i64,
    pub error_message: Option<String>,
    pub max_fee_per_gas_gwei: Option<f64>,
    pub max_priority_fee_per_gas_gwei: Option<f64>,
    pub created_at: DateTime<Utc>,
}

/// Transaction statistics
#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct TransactionStats {
    pub feed_name: String,
    pub network_name: String,
    pub total_transactions: i64,
    pub successful_transactions: i64,
    pub failed_transactions: i64,
    pub error_transactions: i64,
    pub avg_gas_used: Option<f64>,
    pub avg_gas_price_gwei: Option<f64>,
    pub avg_efficiency_percent: Option<f64>,
    pub total_cost_wei: Option<String>,
    pub first_transaction: Option<DateTime<Utc>>,
    pub last_transaction: Option<DateTime<Utc>>,
}

impl TransactionLogRepository {
    /// Create a new repository instance
    pub fn new(pool: DatabasePool) -> Self {
        Self { pool }
    }

    /// Save a transaction log entry
    pub async fn save_transaction(&self, details: TransactionDetails) -> Result<i32> {
        let total_cost_wei = details.total_cost_wei.to_string();

        debug!(
            "Attempting to save transaction log: feed={}, network={}, tx_hash={}, gas_used={}, gas_price={:.2} gwei, status={}, efficiency={:.1}%",
            details.feed_name, details.network, details.tx_hash, details.gas_used, details.gas_price_gwei, details.status, details.efficiency_percent
        );

        let result = sqlx::query_as::<_, (i32,)>(
            r#"
            INSERT INTO transaction_log (
                tx_hash, feed_name, network_name, gas_limit, gas_used,
                gas_price_gwei, total_cost_wei, efficiency_percent,
                tx_type, status, block_number, error_message
            ) VALUES ($1, $2, $3, $4, $5, $6, $7::NUMERIC, $8, $9, $10, $11, $12)
            ON CONFLICT (tx_hash) DO UPDATE SET
                gas_used = EXCLUDED.gas_used,
                gas_price_gwei = EXCLUDED.gas_price_gwei,
                total_cost_wei = EXCLUDED.total_cost_wei,
                efficiency_percent = EXCLUDED.efficiency_percent,
                status = EXCLUDED.status,
                block_number = EXCLUDED.block_number,
                error_message = EXCLUDED.error_message
            RETURNING id
            "#,
        )
        .bind(&details.tx_hash)
        .bind(&details.feed_name)
        .bind(&details.network)
        .bind(details.gas_limit as i64)
        .bind(details.gas_used as i64)
        .bind(details.gas_price_gwei)
        .bind(total_cost_wei)
        .bind(details.efficiency_percent)
        .bind(&details.tx_type)
        .bind(&details.status)
        .bind(details.block_number as i64)
        .bind(&details.error_message)
        .fetch_one(&self.pool)
        .await
        .context("Failed to save transaction log")?;

        debug!(
            "Successfully saved transaction log with id={}: feed={} on {} - tx_hash: {}, block_number={}",
            result.0, details.feed_name, details.network, details.tx_hash, details.block_number
        );

        Ok(result.0)
    }

    /// Get transaction logs for a specific feed
    #[allow(dead_code)]
    pub async fn get_by_feed(
        &self,
        feed_name: &str,
        network_name: &str,
        limit: i64,
    ) -> Result<Vec<TransactionLog>> {
        let logs = sqlx::query_as::<_, TransactionLog>(
            r#"
            SELECT * FROM transaction_log
            WHERE feed_name = $1 AND network_name = $2
            ORDER BY created_at DESC
            LIMIT $3
            "#,
        )
        .bind(feed_name)
        .bind(network_name)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch transaction logs")?;

        Ok(logs)
    }

    /// Get transaction statistics for all feeds
    #[allow(dead_code)]
    pub async fn get_stats(&self) -> Result<Vec<TransactionStats>> {
        let stats = sqlx::query_as::<_, TransactionStats>(
            r#"
            SELECT * FROM transaction_stats
            ORDER BY network_name, feed_name
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch transaction stats")?;

        Ok(stats)
    }

    /// Get daily gas costs for a specific network
    #[allow(dead_code)]
    pub async fn get_daily_costs(
        &self,
        network_name: &str,
        days: i32,
    ) -> Result<Vec<DailyGasCost>> {
        let costs = sqlx::query_as::<_, DailyGasCost>(
            r#"
            SELECT 
                date,
                network_name,
                feed_name,
                transaction_count,
                total_gas_used,
                avg_gas_price_gwei,
                total_cost_wei,
                avg_efficiency_percent
            FROM daily_gas_costs
            WHERE network_name = $1 
                AND date >= CURRENT_DATE - INTERVAL '$2 days'
            ORDER BY date DESC, feed_name
            "#,
        )
        .bind(network_name)
        .bind(days)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch daily gas costs")?;

        Ok(costs)
    }

    /// Get high gas consumption transactions
    #[allow(dead_code)]
    pub async fn get_high_gas_transactions(
        &self,
        threshold_gwei: f64,
        limit: i64,
    ) -> Result<Vec<TransactionLog>> {
        let logs = sqlx::query_as::<_, TransactionLog>(
            r#"
            SELECT * FROM transaction_log
            WHERE gas_price_gwei > $1
            ORDER BY gas_price_gwei DESC, created_at DESC
            LIMIT $2
            "#,
        )
        .bind(threshold_gwei)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch high gas transactions")?;

        Ok(logs)
    }

    /// Get inefficient transactions (low gas efficiency)
    #[allow(dead_code)]
    pub async fn get_inefficient_transactions(
        &self,
        efficiency_threshold: f64,
        limit: i64,
    ) -> Result<Vec<TransactionLog>> {
        let logs = sqlx::query_as::<_, TransactionLog>(
            r#"
            SELECT * FROM transaction_log
            WHERE efficiency_percent < $1 AND status = 'success'
            ORDER BY efficiency_percent ASC, created_at DESC
            LIMIT $2
            "#,
        )
        .bind(efficiency_threshold)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch inefficient transactions")?;

        Ok(logs)
    }

    /// Clean up old transaction logs
    #[allow(dead_code)]
    pub async fn cleanup_old_logs(&self, days_to_keep: i32) -> Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM transaction_log
            WHERE created_at < CURRENT_TIMESTAMP - INTERVAL '$1 days'
            "#,
        )
        .bind(days_to_keep)
        .execute(&self.pool)
        .await
        .context("Failed to cleanup old transaction logs")?;

        let deleted = result.rows_affected();
        if deleted > 0 {
            debug!("Cleaned up {} old transaction logs", deleted);
        }

        Ok(deleted)
    }
}

/// Daily gas cost summary
#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct DailyGasCost {
    pub date: chrono::NaiveDate,
    pub network_name: String,
    pub feed_name: String,
    pub transaction_count: i64,
    pub total_gas_used: i64,
    pub avg_gas_price_gwei: f64,
    pub total_cost_wei: String, // Store as string to avoid BigDecimal issues
    pub avg_efficiency_percent: f64,
}
