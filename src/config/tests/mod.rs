#[cfg(test)]
mod tests {
    use std::io::Write;
    use tempfile::NamedTempFile;
    use crate::config::parser::{load_config, ConfigError};

    // Helper function to create a temporary file with content
    fn create_temp_file(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");
        file.write_all(content.as_bytes()).expect("Failed to write to temp file");
        file.flush().expect("Failed to flush temp file");
        file
    }

    #[test]
    fn test_valid_configuration() {
        let config_yaml = r#"
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

        let temp_file = create_temp_file(config_yaml);
        let config = load_config(temp_file.path()).expect("Failed to load valid config");

        assert_eq!(config.networks.len(), 2);
        assert_eq!(config.datafeeds.len(), 1);
        
        assert_eq!(config.networks[0].name, "ethereum");
        assert_eq!(config.networks[0].rpc_url, "https://eth.llamarpc.com");
        
        assert_eq!(config.datafeeds[0].name, "eth_usd");
        assert_eq!(config.datafeeds[0].networks, "ethereum");
        assert_eq!(config.datafeeds[0].check_frequency, 60);
        assert_eq!(config.datafeeds[0].contract_address, "0x1234567890123456789012345678901234567890");
        assert_eq!(config.datafeeds[0].contract_type, "fluxmon");
        assert!(config.datafeeds[0].read_contract_config);
        assert_eq!(config.datafeeds[0].minimum_update_frequency, 3600);
        assert_eq!(config.datafeeds[0].deviation_threshold_pct, 0.5);
        assert_eq!(config.datafeeds[0].feed_url, "https://min-api.cryptocompare.com/data/pricemultifull?fsyms=ETH&tsyms=USD");
        assert_eq!(config.datafeeds[0].feed_json_path, "RAW.ETH.USD.PRICE");
        assert_eq!(config.datafeeds[0].feed_json_path_timestamp, Some("RAW.ETH.USD.LASTUPDATE".to_string()));
    }

    #[test]
    fn test_minimal_valid_configuration() {
        let config_yaml = r#"
        networks:
          - name: ethereum
            rpc_url: https://eth.llamarpc.com

        datafeeds:
          - name: eth_usd
            networks: ethereum
            check_frequency: 60
            contract_address: 0x1234567890123456789012345678901234567890
            contract_type: fluxmon
            read_contract_config: false
            decimals: 8
            min_value: 0
            max_value: 1000000
            minimum_update_frequency: 3600
            deviation_threshold_pct: 0.5
            feed_url: https://min-api.cryptocompare.com/data/pricemultifull?fsyms=ETH&tsyms=USD
            feed_json_path: RAW.ETH.USD.PRICE
        "#;

        let temp_file = create_temp_file(config_yaml);
        let config = load_config(temp_file.path()).expect("Failed to load valid config");

        assert_eq!(config.networks.len(), 1);
        assert_eq!(config.datafeeds.len(), 1);
        
        assert_eq!(config.datafeeds[0].feed_json_path_timestamp, None);
        assert_eq!(config.datafeeds[0].decimals, Some(8));
        assert_eq!(config.datafeeds[0].min_value, Some(0));
        assert_eq!(config.datafeeds[0].max_value, Some(1000000));
    }

    #[test]
    fn test_invalid_eth_address() {
        let config_yaml = r#"
        networks:
          - name: ethereum
            rpc_url: https://eth.llamarpc.com

        datafeeds:
          - name: eth_usd
            networks: ethereum
            check_frequency: 60
            contract_address: invalid_address
            contract_type: fluxmon
            read_contract_config: true
            minimum_update_frequency: 3600
            deviation_threshold_pct: 0.5
            feed_url: https://min-api.cryptocompare.com/data/pricemultifull?fsyms=ETH&tsyms=USD
            feed_json_path: RAW.ETH.USD.PRICE
        "#;

        let temp_file = create_temp_file(config_yaml);
        let result = load_config(temp_file.path());
        
        assert!(result.is_err());
        // Just check that the validation fails, not exactly how
        assert!(matches!(result, Err(ConfigError::ValidationError(_))));
    }

    #[test]
    fn test_missing_required_fields() {
        let config_yaml = r#"
        networks:
          - name: ethereum
            rpc_url: https://eth.llamarpc.com

        datafeeds:
          - name: eth_usd
            networks: ethereum
            check_frequency: 60
            contract_address: 0x1234567890123456789012345678901234567890
            # Missing contract_type
            read_contract_config: true
            minimum_update_frequency: 3600
            deviation_threshold_pct: 0.5
            # Missing feed_url
            feed_json_path: RAW.ETH.USD.PRICE
        "#;

        let temp_file = create_temp_file(config_yaml);
        let result = load_config(temp_file.path());
        
        assert!(result.is_err());
        // The error should be a parsing error since required fields are missing
        assert!(matches!(result, Err(ConfigError::ParseError(_))));
    }

    #[test]
    fn test_invalid_network_reference() {
        let config_yaml = r#"
        networks:
          - name: ethereum
            rpc_url: https://eth.llamarpc.com

        datafeeds:
          - name: eth_usd
            networks: non_existent_network
            check_frequency: 60
            contract_address: 0x1234567890123456789012345678901234567890
            contract_type: fluxmon
            read_contract_config: true
            minimum_update_frequency: 3600
            deviation_threshold_pct: 0.5
            feed_url: https://min-api.cryptocompare.com/data/pricemultifull?fsyms=ETH&tsyms=USD
            feed_json_path: RAW.ETH.USD.PRICE
        "#;

        let temp_file = create_temp_file(config_yaml);
        let result = load_config(temp_file.path());
        
        assert!(result.is_err());
        assert!(matches!(result, Err(ConfigError::Other(_))));
        if let Err(ConfigError::Other(err)) = result {
            assert!(err.contains("references network 'non_existent_network' which is not defined"));
        } else {
            panic!("Expected Other error for invalid network reference");
        }
    }

    #[test]
    fn test_invalid_url() {
        let config_yaml = r#"
        networks:
          - name: ethereum
            rpc_url: not-a-valid-url

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
        "#;

        let temp_file = create_temp_file(config_yaml);
        let result = load_config(temp_file.path());
        
        assert!(result.is_err());
        // Just check that validation fails, not how
        assert!(matches!(result, Err(ConfigError::ValidationError(_))));
    }

    #[test]
    fn test_invalid_yaml() {
        let config_yaml = r#"
        networks:
          - name: ethereum
            rpc_url: https://eth.llamarpc.com
          - this is not valid yaml
        "#;

        let temp_file = create_temp_file(config_yaml);
        let result = load_config(temp_file.path());
        
        assert!(result.is_err());
        assert!(matches!(result, Err(ConfigError::ParseError(_))));
    }

    #[test]
    fn test_invalid_deviation_threshold() {
        let config_yaml = r#"
        networks:
          - name: ethereum
            rpc_url: https://eth.llamarpc.com

        datafeeds:
          - name: eth_usd
            networks: ethereum
            check_frequency: 60
            contract_address: 0x1234567890123456789012345678901234567890
            contract_type: fluxmon
            read_contract_config: true
            minimum_update_frequency: 3600
            deviation_threshold_pct: 101.0  # Should be between 0 and 100
            feed_url: https://min-api.cryptocompare.com/data/pricemultifull?fsyms=ETH&tsyms=USD
            feed_json_path: RAW.ETH.USD.PRICE
        "#;

        let temp_file = create_temp_file(config_yaml);
        let result = load_config(temp_file.path());
        
        assert!(result.is_err());
        // Just check that validation fails, not how
        assert!(matches!(result, Err(ConfigError::ValidationError(_))));
    }
}