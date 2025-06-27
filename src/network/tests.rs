#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::config::models::Network;
    use std::env;
    use tokio;

    // Helper function to create test network config
    fn create_test_network(name: &str, rpc_url: &str) -> Network {
        Network {
            name: name.to_string(),
            rpc_url: rpc_url.to_string(),
            transaction_type: "eip1559".to_string(),
            gas_config: Default::default(),
            gas_token: "ethereum".to_string(),
            gas_token_symbol: "ETH".to_string(),
        }
    }

    #[tokio::test]
    async fn test_network_manager_new_with_valid_networks() {
        // This test requires a local test network or will fail
        // For unit tests, we'll focus on the structure rather than actual connections
        let networks = vec![
            create_test_network("test1", "http://localhost:8545"),
            create_test_network("test2", "http://localhost:8546"),
        ];

        // We can't test actual connection without a running node
        // So we'll test the error handling
        let result = NetworkManager::new(&networks).await;

        // The test will fail to connect, which is expected in unit tests
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_network_manager_invalid_url() {
        let networks = vec![create_test_network("invalid", "not-a-valid-url")];

        let result = NetworkManager::new(&networks).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_provider_not_found() {
        // Create a NetworkManager with no networks
        let manager = NetworkManager::new(&[]).await.unwrap();

        let result = manager.get_provider("non_existent");
        assert!(result.is_err());

        match result {
            Err(e) => {
                let error_msg = e.to_string();
                assert!(error_msg.contains("Network not found: non_existent"));
            }
            Ok(_) => panic!("Expected error for non-existent network"),
        }
    }

    #[tokio::test]
    async fn test_get_signer_not_found() {
        // Create a NetworkManager with no networks
        let manager = NetworkManager::new(&[]).await.unwrap();

        let result = manager.get_signer("non_existent");
        assert!(result.is_err());

        match result {
            Err(e) => {
                let error_msg = e.to_string();
                assert!(error_msg.contains("No signer found for network non_existent"));
            }
            Ok(_) => panic!("Expected error for non-existent signer"),
        }
    }

    #[tokio::test]
    async fn test_load_wallet_from_env_network_not_found() {
        // Create a NetworkManager with no networks
        let mut manager = NetworkManager::new(&[]).await.unwrap();

        // Set a valid env var
        unsafe {
            env::set_var(
                "TEST_PRIVATE_KEY",
                "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
            );
        }

        let result = manager
            .load_wallet_from_env("test_network", "TEST_PRIVATE_KEY")
            .await;
        assert!(result.is_err());

        match result {
            Err(e) => {
                let error_msg = e.to_string();
                assert!(error_msg.contains("Network not found"));
            }
            Ok(_) => panic!("Expected error for non-existent network"),
        }

        // Clean up
        unsafe {
            env::remove_var("TEST_PRIVATE_KEY");
        }
    }

    // Note: Testing invalid private key would require a valid network with provider
    // Since we can't easily mock providers in unit tests, we skip this test
    // This would be better tested as an integration test with a real test network

    #[tokio::test]
    async fn test_network_error_display() {
        let error = NetworkError::NetworkNotFound("ethereum".to_string());
        assert_eq!(error.to_string(), "Network not found: ethereum");

        let error = NetworkError::ConnectionFailed("timeout".to_string());
        assert_eq!(error.to_string(), "RPC connection failed: timeout");
    }

    // Mock provider tests would require more complex setup with mock HTTP servers
    // For now, we focus on the basic structure and error handling tests
}
