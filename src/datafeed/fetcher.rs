use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::Value;
use tracing::{debug, error};

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
        
        let response = self.client
            .get(url)
            .header("Accept", "application/json")
            .send()
            .await
            .with_context(|| format!("Failed to fetch from URL: {}", url))?;
        
        let status = response.status();
        
        if !status.is_success() {
            error!("HTTP error {}: {}", status.as_u16(), status.canonical_reason().unwrap_or("Unknown"));
            anyhow::bail!("HTTP request failed with status: {}", status);
        }
        
        let json: Value = response
            .json()
            .await
            .with_context(|| "Failed to parse response as JSON")?;
        
        debug!("Successfully fetched and parsed JSON data");
        Ok(json)
    }
}

impl Default for Fetcher {
    fn default() -> Self {
        Self::new()
    }
}