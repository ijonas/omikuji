use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use thiserror::Error;
use validator::Validate;

use super::models::OmikujiConfig;

/// Errors that can occur during configuration parsing
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to open config file: {0}")]
    FileError(#[from] std::io::Error),

    #[error("Failed to parse YAML: {0}")]
    ParseError(#[from] serde_yaml::Error),

    #[error("Configuration validation error: {0}")]
    ValidationError(#[from] validator::ValidationErrors),

    #[error("Configuration error: {0}")]
    Other(String),
}

/// Provides default configuration file path
pub fn default_config_path() -> PathBuf {
    dirs::home_dir()
        .expect("Could not determine home directory")
        .join(".omikuji")
        .join("config.yaml")
}

/// Loads and validates the Omikuji configuration
pub fn load_config<P: AsRef<Path>>(config_path: P) -> Result<OmikujiConfig, ConfigError> {
    // Open the configuration file
    let mut file = File::open(&config_path).map_err(ConfigError::FileError)?;

    // Read the file content
    let mut content = String::new();
    file.read_to_string(&mut content)
        .map_err(ConfigError::FileError)?;

    // Parse YAML
    let config: OmikujiConfig = serde_yaml::from_str(&content).map_err(ConfigError::ParseError)?;

    // Validate the configuration
    config.validate().map_err(ConfigError::ValidationError)?;

    // Check if networks referenced by datafeeds exist
    for datafeed in &config.datafeeds {
        if !config.networks.iter().any(|n| n.name == datafeed.networks) {
            return Err(ConfigError::Other(format!(
                "Datafeed '{}' references network '{}' which is not defined",
                datafeed.name, datafeed.networks
            )));
        }
    }

    // Check if networks referenced by scheduled tasks exist
    for task in &config.scheduled_tasks {
        if !config.networks.iter().any(|n| n.name == task.network) {
            return Err(ConfigError::Other(format!(
                "Scheduled task '{}' references network '{}' which is not defined",
                task.name, task.network
            )));
        }
        
        // Validate the scheduled task
        task.validate().map_err(|e| ConfigError::Other(format!(
            "Scheduled task '{}' validation failed: {}",
            task.name, e
        )))?;
    }

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_config() {
        let config_str = r#"
            networks:
              - name: ethereum
                rpc_url: https://eth.llamarpc.com
              - name: base
                rpc_url: https://base.llamarpc.com

            datafeeds:
              - name: eth_usd
                networks: ethereum
                check_frequency: 60
                contract_address: 0x1234567890123456789012345678901234567890
                contract_type: fluxmon
                read_contract_config: true
                minimum_update_frequency: 3600
                deviation_threshold_pct: 0.5
                feed_url: https://min-api.cryptocompare.com/data/pricemultifull?fsyms=ETH&tsyms=USD
                feed_json_path: RAW.ETH.USD.PRICE
                feed_json_path_timestamp: RAW.ETH.USD.LASTUPDATE
        "#;

        let config: OmikujiConfig = serde_yaml::from_str(config_str).unwrap();
        assert_eq!(config.networks.len(), 2);
        assert_eq!(config.datafeeds.len(), 1);
        assert_eq!(config.networks[0].name, "ethereum");
        assert_eq!(config.datafeeds[0].name, "eth_usd");
    }
}
