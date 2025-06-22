use sqlx::PgPool;
use anyhow::{Result, Context};
use chrono::{DateTime, Utc, Duration};
use tracing::{info, debug};

use super::models::{FeedLog, NewFeedLog};

/// Repository for feed log operations
pub struct FeedLogRepository {
    pool: PgPool,
}

impl FeedLogRepository {
    /// Creates a new repository instance
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Saves a new feed log entry
    pub async fn save(&self, log: NewFeedLog) -> Result<FeedLog> {
        debug!(
            "Attempting to save feed log: feed={}, network={}, value={}, timestamp={}, error_status={:?}, network_error={}",
            log.feed_name, log.network_name, log.feed_value, log.feed_timestamp, log.error_status_code, log.network_error
        );
        
        let record = sqlx::query_as::<_, FeedLog>(
            r#"
            INSERT INTO feed_log (
                feed_name, 
                network_name, 
                feed_value, 
                feed_timestamp, 
                error_status_code, 
                network_error,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, NOW())
            RETURNING 
                id,
                feed_name,
                network_name,
                feed_value,
                feed_timestamp,
                updated_at,
                error_status_code,
                network_error,
                created_at
            "#
        )
        .bind(&log.feed_name)
        .bind(&log.network_name)
        .bind(log.feed_value)
        .bind(log.feed_timestamp)
        .bind(log.error_status_code)
        .bind(log.network_error)
        .fetch_one(&self.pool)
        .await
        .context("Failed to insert feed log")?;

        debug!(
            "Successfully saved feed log with id={}: feed={}, network={}, value={}, timestamp={}, created_at={}",
            record.id, record.feed_name, record.network_name, record.feed_value, record.feed_timestamp, record.created_at
        );

        Ok(record)
    }

    /// Gets the latest feed log entry for a specific feed
    #[allow(dead_code)]
    pub async fn get_latest(&self, feed_name: &str, network_name: &str) -> Result<Option<FeedLog>> {
        let record = sqlx::query_as::<_, FeedLog>(
            r#"
            SELECT 
                id,
                feed_name,
                network_name,
                feed_value,
                feed_timestamp,
                updated_at,
                error_status_code,
                network_error,
                created_at
            FROM feed_log
            WHERE feed_name = $1 AND network_name = $2
            ORDER BY created_at DESC
            LIMIT 1
            "#
        )
        .bind(feed_name)
        .bind(network_name)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get latest feed log")?;

        Ok(record)
    }

    /// Gets feed logs within a time range
    #[allow(dead_code)]
    pub async fn get_by_time_range(
        &self,
        feed_name: &str,
        network_name: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<FeedLog>> {
        let records = sqlx::query_as::<_, FeedLog>(
            r#"
            SELECT 
                id,
                feed_name,
                network_name,
                feed_value,
                feed_timestamp,
                updated_at,
                error_status_code,
                network_error,
                created_at
            FROM feed_log
            WHERE feed_name = $1 
                AND network_name = $2
                AND created_at >= $3 
                AND created_at <= $4
            ORDER BY created_at DESC
            "#
        )
        .bind(feed_name)
        .bind(network_name)
        .bind(start_time)
        .bind(end_time)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get feed logs by time range")?;

        Ok(records)
    }

    /// Counts feed logs for a specific feed
    #[allow(dead_code)]
    pub async fn count(&self, feed_name: &str, network_name: &str) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM feed_log
            WHERE feed_name = $1 AND network_name = $2
            "#
        )
        .bind(feed_name)
        .bind(network_name)
        .fetch_one(&self.pool)
        .await
        .context("Failed to count feed logs")?;

        Ok(row.0)
    }

    /// Deletes feed logs older than the specified number of days
    #[allow(dead_code)]
    pub async fn delete_older_than(&self, feed_name: &str, network_name: &str, days: u32) -> Result<u64> {
        let cutoff_date = Utc::now() - Duration::days(days as i64);
        
        let result = sqlx::query(
            r#"
            DELETE FROM feed_log
            WHERE feed_name = $1 
                AND network_name = $2
                AND created_at < $3
            "#
        )
        .bind(feed_name)
        .bind(network_name)
        .bind(cutoff_date)
        .execute(&self.pool)
        .await
        .context("Failed to delete old feed logs")?;

        let deleted_count = result.rows_affected();
        
        if deleted_count > 0 {
            info!(
                "Deleted {} old feed logs for feed '{}' on network '{}' (older than {} days)",
                deleted_count, feed_name, network_name, days
            );
        }

        Ok(deleted_count)
    }

    /// Deletes all feed logs older than the specified date, regardless of feed
    #[allow(dead_code)]
    pub async fn delete_all_older_than(&self, cutoff_date: DateTime<Utc>) -> Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM feed_log
            WHERE created_at < $1
            "#
        )
        .bind(cutoff_date)
        .execute(&self.pool)
        .await
        .context("Failed to delete old feed logs")?;

        let deleted_count = result.rows_affected();
        
        if deleted_count > 0 {
            info!(
                "Deleted {} old feed logs across all feeds (older than {})",
                deleted_count, cutoff_date
            );
        }

        Ok(deleted_count)
    }

    /// Gets a summary of feed logs grouped by feed and network
    #[allow(dead_code)]
    pub async fn get_summary(&self) -> Result<Vec<FeedSummary>> {
        let summaries = sqlx::query_as::<_, FeedSummary>(
            r#"
            SELECT 
                feed_name,
                network_name,
                COUNT(*) as log_count,
                MIN(created_at) as oldest_log,
                MAX(created_at) as newest_log,
                COUNT(CASE WHEN error_status_code IS NOT NULL OR network_error = true THEN 1 END) as error_count
            FROM feed_log
            GROUP BY feed_name, network_name
            ORDER BY feed_name, network_name
            "#
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get feed summary")?;

        Ok(summaries)
    }
}

/// Summary statistics for a feed
#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct FeedSummary {
    pub feed_name: String,
    pub network_name: String,
    pub log_count: i64,
    pub oldest_log: DateTime<Utc>,
    pub newest_log: DateTime<Utc>,
    pub error_count: i64,
}