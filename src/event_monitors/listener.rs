//! Event listening and subscription logic

use super::models::EventMonitor;
use super::response_handler::ResponseHandler;
use super::webhook_caller::WebhookCaller;
use crate::network::NetworkManager;
use alloy::primitives::Address;
use alloy::providers::Provider;
use alloy::rpc::types::Log;
use anyhow::{Context, Result};
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

        // Group monitors by network for efficiency
        let monitors_by_network = self.group_monitors_by_network();

        for (network_name, network_monitors) in monitors_by_network {
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
            .context(format!("Failed to get network '{network_name}'"))?;

        // For now, use HTTP provider with polling
        // TODO: In production, use WebSocket for better performance
        let provider = self.network_manager.get_provider(&network_name)?;

        let webhook_caller = self.webhook_caller.clone();
        let response_handler = self.response_handler.clone();

        let handle = tokio::spawn(async move {
            if let Err(e) = Self::monitor_network_events(
                provider,
                network_name,
                monitors,
                webhook_caller,
                response_handler,
            )
            .await
            {
                error!("Event monitoring error: {}", e);
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
        let _event_selector = Self::parse_event_selector(&monitor.event_signature)?;

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
                    }
                }
            }
        });

        Ok(handle)
    }

    /// Parse event signature to get the event selector
    fn parse_event_selector(signature: &str) -> Result<alloy::primitives::B256> {
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
        debug!(
            "Processing event for monitor '{}' at block {} (tx: {})",
            monitor.name,
            log.block_number.unwrap_or_default(),
            log.transaction_hash.unwrap_or_default()
        );

        // Decode event data
        let processed_event = Self::decode_event_data(&log, monitor)?;

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
        let response_handler = Arc::new(ResponseHandler::new());

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
}
