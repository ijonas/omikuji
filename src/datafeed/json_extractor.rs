use anyhow::{Context, Result};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::debug;

/// Extracts values from JSON using dot-notation paths
pub struct JsonExtractor;

impl JsonExtractor {
    /// Extracts a float value from JSON using a dot-notation path
    /// 
    /// # Arguments
    /// * `json` - The JSON value to extract from
    /// * `path` - Dot-notation path (e.g., "RAW.ETH.USD.PRICE")
    /// 
    /// # Returns
    /// The extracted float value
    pub fn extract_float(json: &Value, path: &str) -> Result<f64> {
        let components: Vec<&str> = path.split('.').collect();
        let mut current = json;
        
        for (index, component) in components.iter().enumerate() {
            current = current.get(component)
                .with_context(|| {
                    format!(
                        "Failed to extract path component '{}' at position {} in path '{}'",
                        component, index, path
                    )
                })?;
        }
        
        // Try to extract as float
        match current {
            Value::Number(n) => {
                n.as_f64()
                    .with_context(|| format!("Failed to convert number to f64 at path '{}'", path))
            }
            Value::String(s) => {
                s.parse::<f64>()
                    .with_context(|| format!("Failed to parse string '{}' as f64 at path '{}'", s, path))
            }
            _ => {
                anyhow::bail!("Value at path '{}' is not a number or string, found: {:?}", path, current);
            }
        }
    }
    
    /// Extracts a timestamp from JSON using a dot-notation path
    /// 
    /// # Arguments
    /// * `json` - The JSON value to extract from
    /// * `path` - Optional dot-notation path
    /// 
    /// # Returns
    /// The extracted timestamp or current time if path is None
    pub fn extract_timestamp(json: &Value, path: Option<&str>) -> Result<u64> {
        match path {
            Some(p) => {
                let value = Self::extract_float(json, p)?;
                Ok(value as u64)
            }
            None => {
                // Generate current timestamp
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .context("Failed to get current timestamp")?;
                Ok(now.as_secs())
            }
        }
    }
    
    /// Extracts both value and timestamp from JSON
    /// 
    /// # Arguments
    /// * `json` - The JSON value to extract from
    /// * `value_path` - Dot-notation path for the value
    /// * `timestamp_path` - Optional dot-notation path for the timestamp
    /// 
    /// # Returns
    /// Tuple of (value, timestamp)
    pub fn extract_feed_data(
        json: &Value,
        value_path: &str,
        timestamp_path: Option<&str>,
    ) -> Result<(f64, u64)> {
        let value = Self::extract_float(json, value_path)?;
        let timestamp = Self::extract_timestamp(json, timestamp_path)?;
        
        debug!(
            "Extracted feed data: value={}, timestamp={}",
            value, timestamp
        );
        
        Ok((value, timestamp))
    }
}