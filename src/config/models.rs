use alloy::primitives::I256;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

/// The main configuration structure for Omikuji
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct OmikujiConfig {
    /// Networks supported by this Omikuji instance
    #[validate]
    pub networks: Vec<Network>,

    /// Datafeeds managed by this Omikuji instance
    #[validate]
    pub datafeeds: Vec<Datafeed>,

    /// Database cleanup configuration
    #[serde(default)]
    #[validate]
    pub database_cleanup: DatabaseCleanupConfig,
}

/// Configuration for database cleanup task
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct DatabaseCleanupConfig {
    /// Cron schedule for cleanup task (default: "0 0 * * * *" - every hour)
    #[serde(default = "default_cleanup_schedule")]
    pub schedule: String,

    /// Whether cleanup is enabled (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for DatabaseCleanupConfig {
    fn default() -> Self {
        Self {
            schedule: default_cleanup_schedule(),
            enabled: true,
        }
    }
}

fn default_cleanup_schedule() -> String {
    "0 0 * * * *".to_string() // Every hour at minute 0
}

/// Configuration for a blockchain network
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Network {
    /// Network name (e.g., "ethereum", "base")
    #[validate(length(min = 1))]
    pub name: String,

    /// RPC URL for the network
    #[validate(url)]
    pub rpc_url: String,

    /// Transaction type to use ("legacy" or "eip1559")
    #[serde(default = "default_transaction_type")]
    #[validate(custom = "validate_transaction_type")]
    pub transaction_type: String,

    /// Gas configuration for this network
    #[serde(default)]
    #[validate]
    pub gas_config: GasConfig,
}

/// Gas configuration for a network
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct GasConfig {
    /// Gas limit override (optional, will estimate if not provided)
    pub gas_limit: Option<u64>,

    /// For legacy transactions: gas price in gwei (optional, will estimate if not provided)
    pub gas_price_gwei: Option<f64>,

    /// For EIP-1559: max fee per gas in gwei (optional, will estimate if not provided)
    pub max_fee_per_gas_gwei: Option<f64>,

    /// For EIP-1559: max priority fee per gas in gwei (optional, will estimate if not provided)
    pub max_priority_fee_per_gas_gwei: Option<f64>,

    /// Gas estimation multiplier (default: 1.2 for 20% buffer)
    #[serde(default = "default_gas_multiplier")]
    #[validate(range(min = 1.0, max = 5.0))]
    pub gas_multiplier: f64,

    /// Fee bumping configuration
    #[serde(default)]
    #[validate]
    pub fee_bumping: FeeBumpingConfig,
}

/// Fee bumping configuration for stuck transactions
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct FeeBumpingConfig {
    /// Enable automatic fee bumping for stuck transactions
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Maximum number of retry attempts
    #[serde(default = "default_max_retries")]
    #[validate(range(min = 0, max = 10))]
    pub max_retries: u8,

    /// Initial wait time before first retry (in seconds)
    #[serde(default = "default_initial_wait")]
    #[validate(range(min = 10, max = 600))]
    pub initial_wait_seconds: u64,

    /// Fee increase percentage for each retry
    #[serde(default = "default_fee_increase_percent")]
    #[validate(range(min = 5.0, max = 100.0))]
    pub fee_increase_percent: f64,
}

fn default_transaction_type() -> String {
    "eip1559".to_string()
}

fn default_gas_multiplier() -> f64 {
    1.2
}

fn default_true() -> bool {
    true
}

fn default_max_retries() -> u8 {
    3
}

fn default_initial_wait() -> u64 {
    30
}

fn default_fee_increase_percent() -> f64 {
    10.0
}

impl Default for GasConfig {
    fn default() -> Self {
        Self {
            gas_limit: None,
            gas_price_gwei: None,
            max_fee_per_gas_gwei: None,
            max_priority_fee_per_gas_gwei: None,
            gas_multiplier: default_gas_multiplier(),
            fee_bumping: FeeBumpingConfig::default(),
        }
    }
}

impl Default for FeeBumpingConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            max_retries: default_max_retries(),
            initial_wait_seconds: default_initial_wait(),
            fee_increase_percent: default_fee_increase_percent(),
        }
    }
}

/// Configuration for a datafeed
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
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
    pub min_value: Option<I256>,

    /// Maximum valid value (optional, used when read_contract_config is false)
    pub max_value: Option<I256>,

    /// Data retention window in days (default: 7)
    #[serde(default = "default_data_retention_days")]
    #[validate(range(min = 1, max = 365))]
    pub data_retention_days: u32,
}

fn default_data_retention_days() -> u32 {
    7
}

/// Validates that a string is a valid Ethereum address
fn validate_eth_address(address: &str) -> Result<(), ValidationError> {
    // Simple validation: check if it's a hex string starting with 0x and of correct length
    // For more comprehensive validation, we might need to check the checksum
    if !address.starts_with("0x")
        || address.len() != 42
        || !address[2..].chars().all(|c| c.is_ascii_hexdigit())
    {
        return Err(ValidationError::new("invalid_eth_address"));
    }
    Ok(())
}

/// Validates that transaction type is either "legacy" or "eip1559"
pub fn validate_transaction_type(tx_type: &str) -> Result<(), ValidationError> {
    match tx_type.to_lowercase().as_str() {
        "legacy" | "eip1559" => Ok(()),
        _ => Err(ValidationError::new("invalid_transaction_type")),
    }
}
