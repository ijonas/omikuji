use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

/// The main configuration structure for Omikuji
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct OmikujiConfig {
    /// Networks supported by this Omikuji instance
    #[validate]
    pub networks: Vec<Network>,

    /// Datafeeds managed by this Omikuji instance
    #[validate]
    pub datafeeds: Vec<Datafeed>,
}

/// Configuration for a blockchain network
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct Network {
    /// Network name (e.g., "ethereum", "base")
    #[validate(length(min = 1))]
    pub name: String,

    /// RPC URL for the network
    #[validate(url)]
    pub rpc_url: String,
}

/// Configuration for a datafeed
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct Datafeed {
    /// Datafeed name
    #[validate(length(min = 1))]
    pub name: String,

    /// Network this datafeed operates on (must match a network name)
    #[validate(length(min = 1))]
    pub networks: String,

    /// Frequency to check the datafeed (in seconds)
    #[validate(range(min = 1))]
    pub check_frequency: u64,

    /// Smart contract address for the datafeed
    #[validate(custom = "validate_eth_address")]
    pub contract_address: String,

    /// Contract type (e.g., "fluxmon")
    #[validate(length(min = 1))]
    pub contract_type: String,

    /// Whether to read configuration from the contract
    pub read_contract_config: bool,

    /// Minimum time between updates (in seconds)
    #[validate(range(min = 1))]
    pub minimum_update_frequency: u64,

    /// Threshold percentage deviation to trigger an update
    #[validate(range(min = 0.0, max = 100.0))]
    pub deviation_threshold_pct: f64,

    /// URL to fetch the price feed data
    #[validate(url)]
    pub feed_url: String,

    /// JSON path to extract the price from the feed response
    #[validate(length(min = 1))]
    pub feed_json_path: String,

    /// JSON path to extract the timestamp from the feed response (optional)
    pub feed_json_path_timestamp: Option<String>,

    /// Number of decimals to use (optional, used when read_contract_config is false)
    pub decimals: Option<u8>,

    /// Minimum valid value (optional, used when read_contract_config is false)
    pub min_value: Option<i64>,

    /// Maximum valid value (optional, used when read_contract_config is false)
    pub max_value: Option<i64>,
}

/// Validates that a string is a valid Ethereum address
fn validate_eth_address(address: &str) -> Result<(), ValidationError> {
    // Simple validation: check if it's a hex string starting with 0x and of correct length
    // For more comprehensive validation, we might need to check the checksum
    if !address.starts_with("0x") || address.len() != 42 || !address[2..].chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ValidationError::new("invalid_eth_address"));
    }
    Ok(())
}