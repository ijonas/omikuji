use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Represents a single feed value log entry in the database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct FeedLog {
    /// Auto-incrementing internal feed ID
    pub id: i32,

    /// Feed name as defined in config.yaml
    pub feed_name: String,

    /// Network name for the feed
    pub network_name: String,

    /// The value retrieved from the feed
    pub feed_value: f64,

    /// Timestamp as reported by the feed
    pub feed_timestamp: i64,

    /// Timestamp when the system recorded the value
    pub updated_at: DateTime<Utc>,

    /// HTTP status code if different from 200
    pub error_status_code: Option<i32>,

    /// Whether there was a network error (no HTTP response)
    pub network_error: bool,

    /// Timestamp when the record was created
    pub created_at: DateTime<Utc>,
}

/// Parameters for creating a new feed log entry
#[derive(Debug, Clone)]
pub struct NewFeedLog {
    pub feed_name: String,
    pub network_name: String,
    pub feed_value: f64,
    pub feed_timestamp: i64,
    pub error_status_code: Option<i32>,
    pub network_error: bool,
}
