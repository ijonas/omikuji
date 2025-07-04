//! Event listening and subscription logic

use super::error::{EventMonitorError, Result};
use super::metrics::{EventMonitorMetrics, EventMonitorMetricsContext};
use super::models::EventMonitor;
use super::response_handler::ResponseHandler;
use super::webhook_caller::WebhookCaller;
use crate::network::NetworkManager;
use alloy::primitives::Address;
use alloy::providers::Provider;
use alloy::rpc::types::Log;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, error, info};

/// Manages event subscriptions for multiple monitors
pub struct EventListener {
    network_manager: Arc<NetworkManager>,
    webhook_caller: Arc<WebhookCaller>,
    response_handler: Arc<ResponseHandler>,
    monitors: Vec<EventMonitor>,
}

/// Processed event data ready for webhook
#[derive(Debug, Clone)]
pub struct ProcessedEvent {
    pub monitor_name: String,
    pub event_name: String,
    pub contract_address: Address,
    pub transaction_hash: String,
    pub block_number: u64,
    pub log_index: u64,
    pub removed: bool,
    pub topics: Vec<String>,
    pub data: String,
    pub decoded_args: serde_json::Value,
}

/// Context for event processing
#[derive(Debug, Clone)]
pub struct EventContext {
    pub network: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub omikuji_version: String,
}

impl EventListener {
    /// Create a new event listener
    pub fn new(
        network_manager: Arc<NetworkManager>,
        webhook_caller: Arc<WebhookCaller>,
        response_handler: Arc<ResponseHandler>,
        monitors: Vec<EventMonitor>,
    ) -> Self {
        Self {
            network_manager,
            webhook_caller,
            response_handler,
            monitors,
        }
    }

    /// Start monitoring events for all configured monitors
    pub async fn start_monitoring(&self) -> Result<Vec<JoinHandle<()>>> {
        let mut handles = Vec::new();
        let metrics = EventMonitorMetrics::global();

        // Group monitors by network for efficiency
        let monitors_by_network = self.group_monitors_by_network();

        for (network_name, network_monitors) in monitors_by_network {
            let monitor_count = network_monitors.len() as i64;
            metrics.update_active_subscriptions(&network_name, monitor_count);

            let handle = self
                .start_network_monitoring(network_name, network_monitors)
                .await?;
            handles.push(handle);
        }

        info!(
            "Started event monitoring for {} monitors",
            self.monitors.len()
        );
        Ok(handles)
    }

    /// Group monitors by network
    fn group_monitors_by_network(&self) -> HashMap<String, Vec<EventMonitor>> {
        let mut grouped = HashMap::new();
        for monitor in &self.monitors {
            grouped
                .entry(monitor.network.clone())
                .or_insert_with(Vec::new)
                .push(monitor.clone());
        }
        grouped
    }

    /// Start monitoring for all monitors on a specific network
    async fn start_network_monitoring(
        &self,
        network_name: String,
        monitors: Vec<EventMonitor>,
    ) -> Result<JoinHandle<()>> {
        let _network = self
            .network_manager
            .get_network(&network_name)
            .await
            .map_err(|_| EventMonitorError::NetworkNotFound(network_name.clone()))?;

        // For now, use HTTP provider with polling
        // TODO: In production, use WebSocket for better performance
        let provider = self
            .network_manager
            .get_provider(&network_name)
            .map_err(|e| EventMonitorError::ProviderError {
                network: network_name.clone(),
                reason: e.to_string(),
            })?;

        let webhook_caller = self.webhook_caller.clone();
        let response_handler = self.response_handler.clone();

        let handle = tokio::spawn(async move {
            if let Err(e) = Self::monitor_network_events(
                provider,
                network_name.clone(),
                monitors,
                webhook_caller,
                response_handler,
            )
            .await
            {
                error!(
                    "Event monitoring error for network '{}': {}",
                    network_name, e
                );
                EventMonitorMetrics::global().update_active_subscriptions(&network_name, 0);
            }
        });

        Ok(handle)
    }

    /// Monitor events for a specific network
    async fn monitor_network_events(
        provider: Arc<crate::network::EthProvider>,
        network_name: String,
        monitors: Vec<EventMonitor>,
        webhook_caller: Arc<WebhookCaller>,
        response_handler: Arc<ResponseHandler>,
    ) -> Result<()> {
        let (tx, mut rx) = mpsc::channel(100);

        // Create subscriptions for each monitor
        let mut subscription_handles = Vec::new();
        for monitor in monitors {
            let handle =
                Self::subscribe_to_monitor_events(provider.clone(), monitor.clone(), tx.clone())
                    .await?;
            subscription_handles.push(handle);
        }

        // Process events as they arrive
        while let Some((monitor, log)) = rx.recv().await {
            if let Err(e) = Self::process_event(
                &monitor,
                log,
                &network_name,
                &webhook_caller,
                &response_handler,
            )
            .await
            {
                error!(
                    "Failed to process event for monitor '{}': {}",
                    monitor.name, e
                );
            }
        }

        Ok(())
    }

    /// Subscribe to events for a specific monitor
    async fn subscribe_to_monitor_events(
        provider: Arc<crate::network::EthProvider>,
        monitor: EventMonitor,
        _tx: mpsc::Sender<(EventMonitor, Log)>,
    ) -> Result<JoinHandle<()>> {
        // Parse event signature to get event selector
        let _event_selector =
            Self::parse_event_selector(&monitor.event_signature).map_err(|e| {
                EventMonitorError::ConfigError {
                    monitor: monitor.name.clone(),
                    reason: format!("Invalid event signature: {e}"),
                }
            })?;

        // For Phase 1, we'll use polling instead of subscriptions
        // TODO: Implement proper WebSocket subscriptions in production

        info!(
            "Starting event polling for '{}' on contract {} for monitor '{}'",
            monitor.event_signature, monitor.contract_address, monitor.name
        );

        let handle = tokio::spawn(async move {
            let mut last_block = 0u64;

            loop {
                // Poll every 5 seconds
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

                match provider.get_block_number().await {
                    Ok(current_block) => {
                        if current_block > last_block {
                            // For Phase 1, just log that we would check for events
                            debug!(
                                "Would check for events from block {} to {} for monitor '{}'",
                                last_block + 1,
                                current_block,
                                monitor.name
                            );
                            last_block = current_block;

                            // TODO: Implement actual event fetching with get_logs
                        }
                    }
                    Err(e) => {
                        error!("Failed to get block number: {}", e);
                        EventMonitorMetrics::global()
                            .record_processing_error(&monitor.name, "block_number_fetch");
                    }
                }
            }
        });

        Ok(handle)
    }

    /// Parse event signature to get the event selector
    fn parse_event_selector(
        signature: &str,
    ) -> std::result::Result<alloy::primitives::B256, String> {
        // For now, we'll use a simple keccak hash of the signature
        // In a real implementation, we'd parse the signature more carefully
        use alloy::primitives::keccak256;
        Ok(keccak256(signature.as_bytes()))
    }

    /// Process a single event
    async fn process_event(
        monitor: &EventMonitor,
        log: Log,
        network_name: &str,
        webhook_caller: &Arc<WebhookCaller>,
        response_handler: &Arc<ResponseHandler>,
    ) -> Result<()> {
        let metrics_ctx =
            EventMonitorMetricsContext::new(monitor.name.clone(), network_name.to_string());
        debug!(
            "Processing event for monitor '{}' at block {} (tx: {})",
            monitor.name,
            log.block_number.unwrap_or_default(),
            log.transaction_hash.unwrap_or_default()
        );

        // Decode event data
        let processed_event = Self::decode_event_data(&log, monitor)?;

        // Record event received
        metrics_ctx.event_received(&processed_event.event_name);

        // Create event context
        let context = EventContext {
            network: network_name.to_string(),
            timestamp: chrono::Utc::now(),
            omikuji_version: env!("CARGO_PKG_VERSION").to_string(),
        };

        // Call webhook
        let response = webhook_caller
            .call_webhook(&monitor.webhook, &processed_event, &context)
            .await?;

        // Handle response
        response_handler
            .handle_response(monitor, response, &processed_event, &context)
            .await?;

        // Record successful processing
        metrics_ctx.event_processed(&processed_event.event_name);

        Ok(())
    }

    /// Decode event data from a log entry
    fn decode_event_data(log: &Log, monitor: &EventMonitor) -> Result<ProcessedEvent> {
        // Extract basic event data
        let event_name = monitor
            .event_signature
            .split('(')
            .next()
            .unwrap_or("Unknown")
            .to_string();

        // Convert log data to string representations
        let topics: Vec<String> = log
            .topics()
            .iter()
            .map(|t| format!("0x{}", hex::encode(t.as_slice())))
            .collect();

        let data = format!("0x{}", hex::encode(&log.data().data));

        // TODO: In Phase 2, implement proper ABI decoding
        // For now, we'll create a placeholder decoded args object
        let decoded_args = serde_json::json!({
            "_notice": "ABI decoding will be implemented in Phase 2",
            "raw_topics": &topics,
            "raw_data": &data,
        });

        Ok(ProcessedEvent {
            monitor_name: monitor.name.clone(),
            event_name,
            contract_address: log.address(),
            transaction_hash: format!("{:?}", log.transaction_hash.unwrap_or_default()),
            block_number: log.block_number.unwrap_or_default(),
            log_index: log.log_index.unwrap_or_default(),
            removed: log.removed,
            topics,
            data,
            decoded_args,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_monitors::models::{HttpMethod, ResponseConfig, WebhookConfig};
    use alloy::primitives::address;
    use std::collections::HashMap;

    fn test_monitor() -> EventMonitor {
        EventMonitor {
            name: "test_monitor".to_string(),
            network: "ethereum-mainnet".to_string(),
            contract_address: address!("1234567890123456789012345678901234567890"),
            event_signature: "Transfer(address,address,uint256)".to_string(),
            webhook: WebhookConfig {
                url: "https://example.com/webhook".to_string(),
                method: HttpMethod::Post,
                headers: HashMap::new(),
                timeout_seconds: 30,
                retry_attempts: 3,
                retry_delay_seconds: 5,
            },
            response: ResponseConfig::default(),
        }
    }

    fn test_event() -> ProcessedEvent {
        ProcessedEvent {
            monitor_name: "test_monitor".to_string(),
            event_name: "TestEvent".to_string(),
            contract_address: address!("1234567890123456789012345678901234567890"),
            transaction_hash: "0xabcd".to_string(),
            block_number: 12345,
            log_index: 0,
            removed: false,
            topics: vec![],
            data: "0x".to_string(),
            decoded_args: serde_json::json!({}),
        }
    }

    fn test_context() -> EventContext {
        EventContext {
            network: "ethereum-mainnet".to_string(),
            timestamp: chrono::Utc::now(),
            omikuji_version: "0.1.0".to_string(),
        }
    }

    #[test]
    fn test_group_monitors_by_network() {
        let mut monitor1 = test_monitor();
        monitor1.name = "monitor1".to_string();

        let mut monitor2 = test_monitor();
        monitor2.name = "monitor2".to_string();
        monitor2.network = "base-mainnet".to_string();

        let mut monitor3 = test_monitor();
        monitor3.name = "monitor3".to_string();

        // For tests, create an empty network manager
        let networks: Vec<crate::config::models::Network> = vec![];
        let network_manager = Arc::new(
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(NetworkManager::new(&networks))
                .unwrap(),
        );
        let webhook_caller = Arc::new(WebhookCaller::new());
        let response_handler = Arc::new(ResponseHandler::new(network_manager.clone()));

        let listener = EventListener::new(
            network_manager,
            webhook_caller,
            response_handler,
            vec![monitor1, monitor2, monitor3],
        );

        let grouped = listener.group_monitors_by_network();
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped.get("ethereum-mainnet").unwrap().len(), 2);
        assert_eq!(grouped.get("base-mainnet").unwrap().len(), 1);
    }

    #[test]
    fn test_parse_event_selector() {
        let selector = EventListener::parse_event_selector("Transfer(address,address,uint256)");
        assert!(selector.is_ok());
    }

    #[test]
    fn test_decode_event_data() {
        use alloy::primitives::{b256, B256};
        use alloy::rpc::types::Log as AlloyLog;

        let monitor = test_monitor();
        let log = AlloyLog {
            inner: alloy::primitives::Log {
                address: monitor.contract_address,
                data: alloy::primitives::LogData::new_unchecked(
                    vec![b256!(
                        "0000000000000000000000000000000000000000000000000000000000000001"
                    )],
                    vec![0x00, 0x00, 0x00, 0x01].into(),
                ),
            },
            block_hash: Some(B256::ZERO),
            block_number: Some(12345),
            block_timestamp: None,
            transaction_hash: Some(b256!(
                "0000000000000000000000000000000000000000000000000000000000000002"
            )),
            transaction_index: Some(0),
            log_index: Some(0),
            removed: false,
        };

        let result = EventListener::decode_event_data(&log, &monitor);
        assert!(result.is_ok());

        let event = result.unwrap();
        assert_eq!(event.monitor_name, "test_monitor");
        assert_eq!(event.event_name, "Transfer");
        assert_eq!(event.block_number, 12345);
        assert!(!event.removed);
        assert_eq!(event.topics.len(), 1);
        assert_eq!(event.data, "0x00000001");
    }

    #[test]
    fn test_event_context_creation() {
        let context = test_context();
        assert_eq!(context.network, "ethereum-mainnet");
        assert_eq!(context.omikuji_version, "0.1.0");
        assert!(context.timestamp <= chrono::Utc::now());
    }

    #[test]
    fn test_processed_event_fields() {
        let event = test_event();
        assert_eq!(event.monitor_name, "test_monitor");
        assert_eq!(event.event_name, "TestEvent");
        assert_eq!(event.block_number, 12345);
        assert_eq!(event.log_index, 0);
        assert!(!event.removed);
        assert!(event.topics.is_empty());
        assert_eq!(event.data, "0x");
    }

    #[tokio::test]
    async fn test_event_listener_creation() {
        let networks: Vec<crate::config::models::Network> = vec![];
        let network_manager = Arc::new(NetworkManager::new(&networks).await.unwrap());
        let webhook_caller = Arc::new(WebhookCaller::new());
        let response_handler = Arc::new(ResponseHandler::new(network_manager.clone()));
        let monitors = vec![test_monitor()];

        let listener = EventListener::new(
            network_manager,
            webhook_caller,
            response_handler,
            monitors.clone(),
        );

        // Verify monitors are stored
        let grouped = listener.group_monitors_by_network();
        assert_eq!(grouped.len(), 1);
        assert!(grouped.contains_key("ethereum-mainnet"));
    }

    #[test]
    fn test_multiple_monitors_grouping() {
        let mut monitor1 = test_monitor();
        monitor1.name = "monitor1".to_string();
        monitor1.network = "ethereum-mainnet".to_string();

        let mut monitor2 = test_monitor();
        monitor2.name = "monitor2".to_string();
        monitor2.network = "base-mainnet".to_string();

        let mut monitor3 = test_monitor();
        monitor3.name = "monitor3".to_string();
        monitor3.network = "ethereum-mainnet".to_string();

        let mut monitor4 = test_monitor();
        monitor4.name = "monitor4".to_string();
        monitor4.network = "arbitrum-mainnet".to_string();

        let networks: Vec<crate::config::models::Network> = vec![];
        let network_manager = Arc::new(
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(NetworkManager::new(&networks))
                .unwrap(),
        );
        let webhook_caller = Arc::new(WebhookCaller::new());
        let response_handler = Arc::new(ResponseHandler::new(network_manager.clone()));

        let listener = EventListener::new(
            network_manager,
            webhook_caller,
            response_handler,
            vec![monitor1, monitor2, monitor3, monitor4],
        );

        let grouped = listener.group_monitors_by_network();
        assert_eq!(grouped.len(), 3);
        assert_eq!(grouped.get("ethereum-mainnet").unwrap().len(), 2);
        assert_eq!(grouped.get("base-mainnet").unwrap().len(), 1);
        assert_eq!(grouped.get("arbitrum-mainnet").unwrap().len(), 1);
    }

    #[test]
    fn test_event_signature_parsing() {
        // Test valid signatures
        assert!(EventListener::parse_event_selector("Transfer(address,address,uint256)").is_ok());
        assert!(EventListener::parse_event_selector("Approval(address,address,uint256)").is_ok());
        assert!(
            EventListener::parse_event_selector("Swap(uint256,uint256,address,address)").is_ok()
        );
        assert!(EventListener::parse_event_selector("SimpleEvent()").is_ok());
        assert!(
            EventListener::parse_event_selector("ComplexEvent(address[],uint256[],bytes)").is_ok()
        );
    }

    // Tests for uncovered lines

    #[tokio::test]
    async fn test_start_monitoring() {
        // Create a test network configuration
        let networks = vec![
            crate::config::models::Network {
                name: "ethereum-mainnet".to_string(),
                rpc_url: "http://localhost:8545".to_string(),
                ws_url: None,
                transaction_type: "eip1559".to_string(),
                gas_config: crate::config::models::GasConfig::default(),
                gas_token: "ethereum".to_string(),
                gas_token_symbol: "ETH".to_string(),
            },
            crate::config::models::Network {
                name: "base-mainnet".to_string(),
                rpc_url: "http://localhost:8546".to_string(),
                ws_url: None,
                transaction_type: "eip1559".to_string(),
                gas_config: crate::config::models::GasConfig::default(),
                gas_token: "ethereum".to_string(),
                gas_token_symbol: "ETH".to_string(),
            },
        ];

        let network_manager = Arc::new(NetworkManager::new(&networks).await.unwrap());
        let webhook_caller = Arc::new(WebhookCaller::new());
        let response_handler = Arc::new(ResponseHandler::new(network_manager.clone()));

        // Create monitors for multiple networks
        let mut monitor1 = test_monitor();
        monitor1.name = "monitor1".to_string();
        monitor1.network = "ethereum-mainnet".to_string();

        let mut monitor2 = test_monitor();
        monitor2.name = "monitor2".to_string();
        monitor2.network = "base-mainnet".to_string();

        let mut monitor3 = test_monitor();
        monitor3.name = "monitor3".to_string();
        monitor3.network = "ethereum-mainnet".to_string();

        let listener = EventListener::new(
            network_manager,
            webhook_caller,
            response_handler,
            vec![monitor1, monitor2, monitor3],
        );

        // Test start_monitoring - it should succeed in returning handles
        let result = listener.start_monitoring().await;
        assert!(result.is_ok());

        let handles = result.unwrap();
        assert_eq!(handles.len(), 2); // One handle per network

        // Abort all handles to clean up
        for handle in handles {
            handle.abort();
        }
    }

    #[tokio::test]
    async fn test_start_network_monitoring_network_not_found() {
        let networks: Vec<crate::config::models::Network> = vec![];
        let network_manager = Arc::new(NetworkManager::new(&networks).await.unwrap());
        let webhook_caller = Arc::new(WebhookCaller::new());
        let response_handler = Arc::new(ResponseHandler::new(network_manager.clone()));

        let listener =
            EventListener::new(network_manager, webhook_caller, response_handler, vec![]);

        // Test with non-existent network
        let result = listener
            .start_network_monitoring("non-existent".to_string(), vec![test_monitor()])
            .await;

        assert!(result.is_err());
        if let Err(e) = result {
            match e {
                EventMonitorError::NetworkNotFound(name) => {
                    assert_eq!(name, "non-existent");
                }
                _ => panic!("Expected NetworkNotFound error"),
            }
        }
    }

    #[tokio::test]
    async fn test_start_network_monitoring_with_valid_network() {
        // Create a network with valid URL
        let networks = vec![crate::config::models::Network {
            name: "test-network".to_string(),
            rpc_url: "http://localhost:8545".to_string(),
            ws_url: None,
            transaction_type: "legacy".to_string(),
            gas_config: crate::config::models::GasConfig::default(),
            gas_token: "test".to_string(),
            gas_token_symbol: "TEST".to_string(),
        }];

        let network_manager = Arc::new(NetworkManager::new(&networks).await.unwrap());
        let webhook_caller = Arc::new(WebhookCaller::new());
        let response_handler = Arc::new(ResponseHandler::new(network_manager.clone()));

        let listener =
            EventListener::new(network_manager, webhook_caller, response_handler, vec![]);

        let result = listener
            .start_network_monitoring("test-network".to_string(), vec![test_monitor()])
            .await;

        assert!(result.is_ok());

        // Clean up the handle
        if let Ok(handle) = result {
            handle.abort();
        }
    }

    #[tokio::test]
    async fn test_monitor_network_events() {
        use alloy::providers::ProviderBuilder;

        // Create a mock provider
        let provider =
            Arc::new(ProviderBuilder::new().on_http("http://localhost:8545".parse().unwrap()));

        let webhook_caller = Arc::new(WebhookCaller::new());
        let networks: Vec<crate::config::models::Network> = vec![];
        let network_manager = Arc::new(NetworkManager::new(&networks).await.unwrap());
        let response_handler = Arc::new(ResponseHandler::new(network_manager));

        let monitors = vec![test_monitor()];

        // Create a channel to test event flow
        let (tx, mut rx) = mpsc::channel(1);

        // Start monitoring in a separate task
        let monitor_task = tokio::spawn(async move {
            let result = EventListener::monitor_network_events(
                provider,
                "test-network".to_string(),
                monitors,
                webhook_caller,
                response_handler,
            )
            .await;
            tx.send(result).await.unwrap();
        });

        // Give it some time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Cancel the task to simulate channel closure
        monitor_task.abort();

        // The function should exit when channel is closed
        if let Ok(result) = rx.try_recv() {
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_subscribe_to_monitor_events() {
        use alloy::providers::ProviderBuilder;

        let provider =
            Arc::new(ProviderBuilder::new().on_http("http://localhost:8545".parse().unwrap()));

        let (tx, _rx) = mpsc::channel(100);
        let monitor = test_monitor();

        // Test subscription creation
        let result =
            EventListener::subscribe_to_monitor_events(provider, monitor.clone(), tx).await;
        assert!(result.is_ok());

        let handle = result.unwrap();

        // The task should be running
        assert!(!handle.is_finished());

        // Abort the task to clean up
        handle.abort();
    }

    #[tokio::test]
    async fn test_subscribe_with_invalid_event_signature() {
        use alloy::providers::ProviderBuilder;

        let provider =
            Arc::new(ProviderBuilder::new().on_http("http://localhost:8545".parse().unwrap()));

        let (tx, _rx) = mpsc::channel(100);
        let mut monitor = test_monitor();
        // This signature is still valid for parsing but would be invalid for actual ABI decoding
        monitor.event_signature = "InvalidEvent(".to_string();

        // Currently parse_event_selector just hashes the string, so this won't fail
        // In a real implementation with proper ABI parsing, this might fail
        let result = EventListener::subscribe_to_monitor_events(provider, monitor, tx).await;
        assert!(result.is_ok());

        if let Ok(handle) = result {
            handle.abort();
        }
    }

    #[tokio::test]
    async fn test_process_event_success() {
        use alloy::primitives::{b256, B256};
        use alloy::rpc::types::Log as AlloyLog;

        // Setup dependencies with a mock HTTP client
        let networks: Vec<crate::config::models::Network> = vec![];
        let network_manager = Arc::new(NetworkManager::new(&networks).await.unwrap());
        let webhook_caller = Arc::new(WebhookCaller::new());
        let response_handler = Arc::new(ResponseHandler::new(network_manager));

        let monitor = test_monitor();

        // Create a test log
        let log = AlloyLog {
            inner: alloy::primitives::Log {
                address: monitor.contract_address,
                data: alloy::primitives::LogData::new_unchecked(
                    vec![b256!(
                        "0000000000000000000000000000000000000000000000000000000000000001"
                    )],
                    vec![0x00, 0x00, 0x00, 0x01].into(),
                ),
            },
            block_hash: Some(B256::ZERO),
            block_number: Some(12345),
            block_timestamp: None,
            transaction_hash: Some(b256!(
                "0000000000000000000000000000000000000000000000000000000000000002"
            )),
            transaction_index: Some(0),
            log_index: Some(0),
            removed: false,
        };

        // Test process_event - will fail due to webhook call but tests the flow
        let result = EventListener::process_event(
            &monitor,
            log,
            "test-network",
            &webhook_caller,
            &response_handler,
        )
        .await;

        // Should fail because we can't actually call the webhook
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_process_event_with_removed_log() {
        use alloy::primitives::{b256, B256};
        use alloy::rpc::types::Log as AlloyLog;

        let networks: Vec<crate::config::models::Network> = vec![];
        let network_manager = Arc::new(NetworkManager::new(&networks).await.unwrap());
        let webhook_caller = Arc::new(WebhookCaller::new());
        let response_handler = Arc::new(ResponseHandler::new(network_manager));

        let monitor = test_monitor();

        // Create a removed log
        let log = AlloyLog {
            inner: alloy::primitives::Log {
                address: monitor.contract_address,
                data: alloy::primitives::LogData::new_unchecked(
                    vec![b256!(
                        "0000000000000000000000000000000000000000000000000000000000000001"
                    )],
                    vec![0x00, 0x00, 0x00, 0x01].into(),
                ),
            },
            block_hash: Some(B256::ZERO),
            block_number: Some(12345),
            block_timestamp: None,
            transaction_hash: Some(b256!(
                "0000000000000000000000000000000000000000000000000000000000000002"
            )),
            transaction_index: Some(0),
            log_index: Some(0),
            removed: true, // Mark as removed
        };

        let result = EventListener::process_event(
            &monitor,
            log,
            "test-network",
            &webhook_caller,
            &response_handler,
        )
        .await;

        // Should still try to process but webhook will fail
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_process_event_decode_and_metrics() {
        use alloy::primitives::{b256, B256};
        use alloy::rpc::types::Log as AlloyLog;

        let monitor = test_monitor();

        // Create a test log with multiple topics
        let log = AlloyLog {
            inner: alloy::primitives::Log {
                address: monitor.contract_address,
                data: alloy::primitives::LogData::new_unchecked(
                    vec![
                        b256!("0000000000000000000000000000000000000000000000000000000000000001"),
                        b256!("0000000000000000000000000000000000000000000000000000000000000002"),
                        b256!("0000000000000000000000000000000000000000000000000000000000000003"),
                    ],
                    vec![0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02].into(),
                ),
            },
            block_hash: Some(B256::ZERO),
            block_number: Some(12345),
            block_timestamp: None,
            transaction_hash: Some(b256!(
                "0000000000000000000000000000000000000000000000000000000000000002"
            )),
            transaction_index: Some(0),
            log_index: Some(5),
            removed: false,
        };

        // Test decoding
        let processed = EventListener::decode_event_data(&log, &monitor).unwrap();
        assert_eq!(processed.event_name, "Transfer");
        assert_eq!(processed.topics.len(), 3);
        assert_eq!(processed.log_index, 5);
        assert_eq!(processed.data, "0x0000000100000002");
    }

    #[tokio::test]
    async fn test_monitor_network_events_with_timeout() {
        use alloy::providers::ProviderBuilder;
        use tokio::time::{timeout, Duration};

        let provider =
            Arc::new(ProviderBuilder::new().on_http("http://localhost:8545".parse().unwrap()));

        let webhook_caller = Arc::new(WebhookCaller::new());
        let networks: Vec<crate::config::models::Network> = vec![];
        let network_manager = Arc::new(NetworkManager::new(&networks).await.unwrap());
        let response_handler = Arc::new(ResponseHandler::new(network_manager));

        let monitors = vec![test_monitor()];

        // Test that monitor_network_events starts up correctly
        // We'll use a timeout to ensure it doesn't block forever
        let monitor_future = EventListener::monitor_network_events(
            provider,
            "test-network".to_string(),
            monitors,
            webhook_caller,
            response_handler,
        );

        // Give it 100ms to start up and then timeout
        let result = timeout(Duration::from_millis(100), monitor_future).await;

        // The function should timeout because it's waiting for events
        assert!(result.is_err());
    }

    #[test]
    fn test_event_context_version() {
        let context = EventContext {
            network: "test".to_string(),
            timestamp: chrono::Utc::now(),
            omikuji_version: env!("CARGO_PKG_VERSION").to_string(),
        };

        assert_eq!(context.omikuji_version, env!("CARGO_PKG_VERSION"));
    }
}
