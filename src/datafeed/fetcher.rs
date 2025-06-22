use anyhow::Result;
use reqwest::Client;
use serde_json::Value;
use thiserror::Error;
use tracing::{debug, error};

/// Errors that can occur when fetching data
#[derive(Debug, Error)]
pub enum FetchError {
    #[error("HTTP error with status code: {0}")]
    Http(u16),

    #[error("Network error: {0}")]
    Network(String),

    #[error("JSON parsing error: {0}")]
    Json(String),
}

/// Fetches JSON data from a given URL
pub struct Fetcher {
    client: Client,
}

impl Fetcher {
    /// Creates a new Fetcher with a reusable HTTP client
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }

    /// Fetches JSON data from the specified URL
    /// Returns the parsed JSON value on success
    pub async fn fetch_json(&self, url: &str) -> Result<Value> {
        debug!("Fetching data from: {}", url);

        let response = match self
            .client
            .get(url)
            .header("Accept", "application/json")
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                error!("Network error fetching from {}: {}", url, e);
                return Err(FetchError::Network(e.to_string()).into());
            }
        };

        let status = response.status();

        if !status.is_success() {
            error!(
                "HTTP error {}: {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or("Unknown")
            );
            return Err(FetchError::Http(status.as_u16()).into());
        }

        let json: Value = match response.json().await {
            Ok(json) => json,
            Err(e) => {
                error!("JSON parsing error: {}", e);
                return Err(FetchError::Json(e.to_string()).into());
            }
        };

        debug!("Successfully fetched and parsed JSON data");
        Ok(json)
    }
}

impl Default for Fetcher {
    fn default() -> Self {
        Self::new()
    }
}
