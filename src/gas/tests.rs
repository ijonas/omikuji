#[cfg(test)]
mod gas_estimator_tests {
    use super::super::estimator::{GasEstimate, GasEstimator};
    use crate::config::models::{FeeBumpingConfig, GasConfig, Network};
    use alloy::primitives::U256;
    use std::sync::Arc;

    fn create_test_network(tx_type: &str) -> Network {
        Network {
            name: "test".to_string(),
            rpc_url: "http://localhost:8545".to_string(),
            transaction_type: tx_type.to_string(),
            gas_config: GasConfig {
                gas_limit: None,
                gas_price_gwei: None,
                max_fee_per_gas_gwei: None,
                max_priority_fee_per_gas_gwei: None,
                gas_multiplier: 1.2,
                fee_bumping: FeeBumpingConfig {
                    enabled: true,
                    max_retries: 3,
                    initial_wait_seconds: 30,
                    fee_increase_percent: 10.0,
                },
            },
        }
    }

    fn create_test_network_with_overrides(
        tx_type: &str,
        gas_limit: Option<u64>,
        gas_price_gwei: Option<f64>,
        max_fee_gwei: Option<f64>,
        priority_fee_gwei: Option<f64>,
    ) -> Network {
        let mut network = create_test_network(tx_type);
        network.gas_config.gas_limit = gas_limit;
        network.gas_config.gas_price_gwei = gas_price_gwei;
        network.gas_config.max_fee_per_gas_gwei = max_fee_gwei;
        network.gas_config.max_priority_fee_per_gas_gwei = priority_fee_gwei;
        network
    }

    #[test]
    fn test_gas_estimate_creation() {
        // Test legacy gas estimate
        let legacy_estimate = GasEstimate {
            gas_limit: U256::from(100_000),
            gas_price: Some(U256::from(20_000_000_000u64)), // 20 gwei
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        };

        assert_eq!(legacy_estimate.gas_limit, U256::from(100_000));
        assert_eq!(
            legacy_estimate.gas_price,
            Some(U256::from(20_000_000_000u64))
        );
        assert!(legacy_estimate.max_fee_per_gas.is_none());
        assert!(legacy_estimate.max_priority_fee_per_gas.is_none());

        // Test EIP-1559 gas estimate
        let eip1559_estimate = GasEstimate {
            gas_limit: U256::from(100_000),
            gas_price: None,
            max_fee_per_gas: Some(U256::from(50_000_000_000u64)), // 50 gwei
            max_priority_fee_per_gas: Some(U256::from(2_000_000_000u64)), // 2 gwei
        };

        assert_eq!(eip1559_estimate.gas_limit, U256::from(100_000));
        assert!(eip1559_estimate.gas_price.is_none());
        assert_eq!(
            eip1559_estimate.max_fee_per_gas,
            Some(U256::from(50_000_000_000u64))
        );
        assert_eq!(
            eip1559_estimate.max_priority_fee_per_gas,
            Some(U256::from(2_000_000_000u64))
        );
    }

    // TODO: Re-enable this test once we have a proper mock provider for alloy
    // #[test]
    // fn test_fee_bumping() {
    //     let network = create_test_network("legacy");
    //     let provider = Arc::new(Provider::<MockProvider>::new(MockProvider::new()));
    //     let estimator = GasEstimator::new(provider, network);

    //     let original = GasEstimate {
    //         gas_limit: U256::from(100_000),
    //         gas_price: Some(U256::from(20_000_000_000u64)), // 20 gwei
    //         max_fee_per_gas: None,
    //         max_priority_fee_per_gas: None,
    //     };

    //     // Test first retry (10% increase)
    //     let bumped1 = estimator.bump_fees(&original, 1);
    //     assert_eq!(bumped1.gas_limit, original.gas_limit); // Gas limit stays same
    //     assert_eq!(
    //         bumped1.gas_price,
    //         Some(U256::from(22_000_000_000u64)) // 22 gwei (10% increase)
    //     );

    //     // Test second retry (20% increase)
    //     let bumped2 = estimator.bump_fees(&original, 2);
    //     assert_eq!(
    //         bumped2.gas_price,
    //         Some(U256::from(24_000_000_000u64)) // 24 gwei (20% increase)
    //     );

    //     // Test third retry (30% increase)
    //     let bumped3 = estimator.bump_fees(&original, 3);
    //     assert_eq!(
    //         bumped3.gas_price,
    //         Some(U256::from(26_000_000_000u64)) // 26 gwei (30% increase)
    //     );
    // }

    // TODO: Re-enable this test once we have a proper mock provider for alloy
    // #[test]
    // fn test_fee_bumping_eip1559() {
    //     let network = create_test_network("eip1559");
    //     let provider = Arc::new(Provider::<MockProvider>::new(MockProvider::new()));
    //     let estimator = GasEstimator::new(provider, network);

    //     let original = GasEstimate {
    //         gas_limit: U256::from(100_000),
    //         gas_price: None,
    //         max_fee_per_gas: Some(U256::from(50_000_000_000u64)), // 50 gwei
    //         max_priority_fee_per_gas: Some(U256::from(2_000_000_000u64)), // 2 gwei
    //     };

    //     // Test first retry (10% increase)
    //     let bumped1 = estimator.bump_fees(&original, 1);
    //     assert_eq!(bumped1.gas_limit, original.gas_limit);
    //     assert_eq!(
    //         bumped1.max_fee_per_gas,
    //         Some(U256::from(55_000_000_000u64)) // 55 gwei
    //     );
    //     assert_eq!(
    //         bumped1.max_priority_fee_per_gas,
    //         Some(U256::from(2_200_000_000u64)) // 2.2 gwei
    //     );
    // }

    #[test]
    fn test_manual_gas_limit_override() {
        let network = create_test_network_with_overrides(
            "legacy",
            Some(300_000), // Manual gas limit
            None,
            None,
            None,
        );

        // In a real test, we'd need to mock the provider's response
        // This test verifies the configuration is properly set
        assert_eq!(network.gas_config.gas_limit, Some(300_000));
    }

    #[test]
    fn test_manual_gas_price_override() {
        let network = create_test_network_with_overrides(
            "legacy",
            None,
            Some(25.5), // 25.5 gwei
            None,
            None,
        );

        assert_eq!(network.gas_config.gas_price_gwei, Some(25.5));
    }

    #[test]
    fn test_manual_eip1559_override() {
        let network = create_test_network_with_overrides(
            "eip1559",
            None,
            None,
            Some(100.0), // 100 gwei max fee
            Some(5.0),   // 5 gwei priority fee
        );

        assert_eq!(network.gas_config.max_fee_per_gas_gwei, Some(100.0));
        assert_eq!(network.gas_config.max_priority_fee_per_gas_gwei, Some(5.0));
    }

    #[test]
    fn test_gas_multiplier() {
        let mut network = create_test_network("legacy");
        network.gas_config.gas_multiplier = 1.5;

        // Original gas estimate: 100,000
        let original_gas = U256::from(100_000);
        let with_multiplier = original_gas.saturating_mul(U256::from(1500)) / U256::from(1000);

        assert_eq!(with_multiplier, U256::from(150_000));
    }

    #[test]
    fn test_fee_bumping_config() {
        let network = create_test_network("legacy");

        assert!(network.gas_config.fee_bumping.enabled);
        assert_eq!(network.gas_config.fee_bumping.max_retries, 3);
        assert_eq!(network.gas_config.fee_bumping.initial_wait_seconds, 30);
        assert_eq!(network.gas_config.fee_bumping.fee_increase_percent, 10.0);
    }

    #[test]
    fn test_transaction_type_validation() {
        use crate::config::models::validate_transaction_type;

        // Valid types
        assert!(validate_transaction_type("legacy").is_ok());
        assert!(validate_transaction_type("eip1559").is_ok());
        assert!(validate_transaction_type("LEGACY").is_ok());
        assert!(validate_transaction_type("EIP1559").is_ok());

        // Invalid types
        assert!(validate_transaction_type("invalid").is_err());
        assert!(validate_transaction_type("").is_err());
        assert!(validate_transaction_type("eip-1559").is_err());
    }
}
