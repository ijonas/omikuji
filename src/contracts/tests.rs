#[cfg(test)]
mod tests {
    use super::super::flux_aggregator::*;
    use alloy::{
        node_bindings::Anvil,
        primitives::{Address, I256, U256},
        providers::{Provider, ProviderBuilder, RootProvider},
        sol_types::SolCall,
        transports::http::{Client, Http},
    };
    use std::sync::Arc;

    // Helper to deploy a mock FluxAggregator for testing
    // In real tests, you'd deploy an actual contract on a test network
    async fn setup_test_contract() -> (RootProvider<Http<Client>>, Address) {
        // Start a local Anvil instance (Ethereum development node)
        let anvil = Anvil::new().spawn();

        // Connect to the Anvil instance
        let provider = ProviderBuilder::new().on_http(anvil.endpoint_url());

        // In a real test, you would deploy the FluxAggregator contract here
        // For now, we'll use a dummy address
        let contract_address = "0x0000000000000000000000000000000000000001"
            .parse::<Address>()
            .unwrap();

        (provider, contract_address)
    }

    #[tokio::test]
    async fn test_flux_aggregator_instantiation() {
        let (provider, contract_address) = setup_test_contract().await;

        // Create contract instance
        let contract = FluxAggregatorContract::new(contract_address, provider);

        // The contract instance is created successfully
        // We can't directly access the address field as it's private
    }

    #[tokio::test]
    async fn test_contract_call_encoding() {
        let (provider, contract_address) = setup_test_contract().await;
        let contract = FluxAggregatorContract::new(contract_address, provider);

        // Test that method call data can be encoded
        let decimals_call = IFluxAggregator::decimalsCall {};
        let encoded = decimals_call.abi_encode();
        assert!(!encoded.is_empty());

        let submit_call = IFluxAggregator::submitCall {
            _roundId: U256::from(1),
            _submission: I256::try_from(12345).unwrap(),
        };
        let encoded_submit = submit_call.abi_encode();
        assert!(!encoded_submit.is_empty());
    }

    #[test]
    fn test_value_scaling() {
        use crate::datafeed::contract_utils::scale_value_for_contract;

        // Test scaling with 8 decimals
        let value = 1234.56789;
        let scaled = scale_value_for_contract(value, 8);
        assert_eq!(scaled, 123456789000); // 1234.56789 * 10^8 = 123456789000

        // Test scaling with 6 decimals
        let scaled_6 = scale_value_for_contract(value, 6);
        assert_eq!(scaled_6, 1234567890); // 1234.56789 * 10^6 = 1234567890 (rounded)

        // Test scaling with 18 decimals (common for ETH values)
        let eth_value = 1.5;
        let scaled_18 = scale_value_for_contract(eth_value, 18);
        assert_eq!(scaled_18, 1_500_000_000_000_000_000);
    }

    #[test]
    fn test_address_parsing() {
        use crate::datafeed::contract_utils::parse_address;

        // Valid address
        let valid_addr = "0x1234567890123456789012345678901234567890";
        let parsed = parse_address(valid_addr);
        assert!(parsed.is_ok());

        // Invalid address (too short)
        let invalid_addr = "0x123456";
        let parsed_invalid = parse_address(invalid_addr);
        assert!(parsed_invalid.is_err());

        // Address without 0x prefix (alloy accepts this)
        let no_prefix = "1234567890123456789012345678901234567890";
        let parsed_no_prefix = parse_address(no_prefix);
        assert!(parsed_no_prefix.is_ok()); // alloy's Address parser accepts addresses without 0x prefix
    }
}
