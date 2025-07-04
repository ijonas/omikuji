//! Response handling framework for webhook responses

use super::abi_decoder::AbiDecoder;
use super::error::{EventMonitorError, Result};
use super::listener::{EventContext, ProcessedEvent};
use super::metrics::EventMonitorMetricsContext;
use super::models::{EventMonitor, ResponseType};
use super::webhook_caller::{ContractCall, WebhookResponse};
use crate::gas::transaction_builder::GasAwareTransactionBuilder;
use crate::metrics::{MetricsContext, TimedOperationRecorder, TransactionMetricsRecorder};
use crate::network::{EthProvider, NetworkManager};
use crate::utils::{TransactionContext, TransactionLogger};
use alloy::primitives::{Address, U256};
use alloy::providers::Provider;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, warn};

/// Handles webhook responses based on configured response type
pub struct ResponseHandler {
    handlers: HashMap<ResponseType, Arc<dyn Handler>>,
    _network_manager: Arc<NetworkManager>,
}

/// Trait for response handlers
#[async_trait]
pub trait Handler: Send + Sync {
    /// Handle the webhook response
    async fn handle(
        &self,
        monitor: &EventMonitor,
        response: WebhookResponse,
        event: &ProcessedEvent,
        context: &EventContext,
    ) -> Result<()>;
}

/// Handler that only logs the response
pub struct LogOnlyHandler;

/// Handler for contract calls
pub struct ContractCallHandler {
    network_manager: Arc<NetworkManager>,
}

/// Handler for database storage (placeholder for Phase 4)
pub struct StoreDbHandler;

/// Handler for multiple actions
pub struct MultiActionHandler {
    handlers: Vec<Arc<dyn Handler>>,
}

impl ResponseHandler {
    /// Create a new response handler with default handlers
    pub fn new(network_manager: Arc<NetworkManager>) -> Self {
        let mut handlers: HashMap<ResponseType, Arc<dyn Handler>> = HashMap::new();

        handlers.insert(ResponseType::LogOnly, Arc::new(LogOnlyHandler));
        handlers.insert(
            ResponseType::ContractCall,
            Arc::new(ContractCallHandler {
                network_manager: network_manager.clone(),
            }),
        );
        handlers.insert(ResponseType::StoreDb, Arc::new(StoreDbHandler));
        handlers.insert(
            ResponseType::MultiAction,
            Arc::new(MultiActionHandler {
                handlers: vec![Arc::new(LogOnlyHandler), Arc::new(StoreDbHandler)],
            }),
        );

        Self {
            handlers,
            _network_manager: network_manager,
        }
    }

    /// Handle a webhook response
    pub async fn handle_response(
        &self,
        monitor: &EventMonitor,
        response: WebhookResponse,
        event: &ProcessedEvent,
        context: &EventContext,
    ) -> Result<()> {
        let response_type = &monitor.response.response_type;
        let metrics_ctx =
            EventMonitorMetricsContext::new(monitor.name.clone(), context.network.clone());

        debug!(
            "Handling {} response for monitor '{}'",
            match response_type {
                ResponseType::LogOnly => "log-only",
                ResponseType::ContractCall => "contract-call",
                ResponseType::StoreDb => "store-db",
                ResponseType::MultiAction => "multi-action",
            },
            monitor.name
        );

        let response_type_str = match response_type {
            ResponseType::LogOnly => "log_only",
            ResponseType::ContractCall => "contract_call",
            ResponseType::StoreDb => "store_db",
            ResponseType::MultiAction => "multi_action",
        };

        let handler =
            self.handlers
                .get(response_type)
                .ok_or_else(|| EventMonitorError::HandlerError {
                    monitor: monitor.name.clone(),
                    reason: format!("No handler found for response type {response_type:?}"),
                })?;

        let result = handler.handle(monitor, response, event, context).await;

        match &result {
            Ok(_) => metrics_ctx.response_handler_execution(response_type_str, true),
            Err(_) => metrics_ctx.response_handler_execution(response_type_str, false),
        }

        result
    }

    /// Register a custom handler for a response type
    pub fn register_handler(&mut self, response_type: ResponseType, handler: Arc<dyn Handler>) {
        self.handlers.insert(response_type, handler);
    }
}

#[async_trait]
impl Handler for LogOnlyHandler {
    async fn handle(
        &self,
        monitor: &EventMonitor,
        response: WebhookResponse,
        event: &ProcessedEvent,
        _context: &EventContext,
    ) -> Result<()> {
        info!(
            "Webhook response for monitor '{}' (event: {} at block {}): action={}, metadata={:?}",
            monitor.name, event.event_name, event.block_number, response.action, response.metadata
        );

        debug!("Full webhook response: {:?}", response);

        Ok(())
    }
}

#[async_trait]
impl Handler for ContractCallHandler {
    async fn handle(
        &self,
        monitor: &EventMonitor,
        response: WebhookResponse,
        event: &ProcessedEvent,
        context: &EventContext,
    ) -> Result<()> {
        if response.action != "contract_call" {
            warn!(
                "Expected 'contract_call' action but got '{}' for monitor '{}'",
                response.action, monitor.name
            );
            return Ok(());
        }

        let calls = response
            .calls
            .ok_or_else(|| EventMonitorError::HandlerError {
                monitor: monitor.name.clone(),
                reason: "No contract calls provided in response".to_string(),
            })?;

        info!(
            "Executing {} contract calls for monitor '{}' (event: {} at block {})",
            calls.len(),
            monitor.name,
            event.event_name,
            event.block_number
        );

        // Get provider and network config
        let provider = self
            .network_manager
            .get_provider(&context.network)
            .map_err(|e| EventMonitorError::HandlerError {
                monitor: monitor.name.clone(),
                reason: format!("Failed to get provider: {e}"),
            })?;

        let network_config = self
            .network_manager
            .get_network(&context.network)
            .await
            .map_err(|e| EventMonitorError::HandlerError {
                monitor: monitor.name.clone(),
                reason: format!("Failed to get network config: {e}"),
            })?;

        // Get contract call config
        let call_config = monitor.response.contract_call.as_ref().ok_or_else(|| {
            EventMonitorError::HandlerError {
                monitor: monitor.name.clone(),
                reason: "Missing contract call configuration".to_string(),
            }
        })?;

        // Execute each call
        for (i, call) in calls.iter().enumerate() {
            self.execute_contract_call(
                monitor,
                call,
                i,
                calls.len(),
                &provider,
                network_config,
                call_config,
                event,
                context,
            )
            .await?;
        }

        Ok(())
    }
}

impl ContractCallHandler {
    /// Execute a single contract call
    #[allow(clippy::too_many_arguments)]
    async fn execute_contract_call(
        &self,
        monitor: &EventMonitor,
        call: &ContractCall,
        index: usize,
        total: usize,
        provider: &Arc<EthProvider>,
        network_config: &crate::config::models::Network,
        call_config: &crate::event_monitors::models::ContractCallConfig,
        event: &ProcessedEvent,
        context: &EventContext,
    ) -> Result<()> {
        info!(
            "Executing contract call {}/{} for monitor '{}': {} on {}",
            index + 1,
            total,
            monitor.name,
            call.function,
            call.target
        );

        // Create metrics context
        let metrics_ctx = MetricsContext::new(&monitor.name, &context.network);

        // Parse target address
        let target_address =
            call.target
                .parse::<Address>()
                .map_err(|e| EventMonitorError::HandlerError {
                    monitor: monitor.name.clone(),
                    reason: format!("Invalid target address '{}': {}", call.target, e),
                })?;

        // Encode function call
        let call_data =
            AbiDecoder::encode_function_call(&call.function, &call.params, &monitor.name)?;

        // Parse value if provided
        let value = if call.value.is_empty() || call.value == "0" {
            U256::ZERO
        } else {
            U256::from_str_radix(&call.value, 10).map_err(|e| EventMonitorError::HandlerError {
                monitor: monitor.name.clone(),
                reason: format!("Invalid value '{}': {}", call.value, e),
            })?
        };

        // Record the attempt
        let recorder = TimedOperationRecorder::contract_write(metrics_ctx.clone());

        // Create transaction context
        let _tx_context = TransactionContext::EventMonitor {
            monitor_name: monitor.name.clone(),
            event_name: event.event_name.clone(),
            function_name: call.function.clone(),
        };

        // Log submission
        let value_str = if value.is_zero() {
            None
        } else {
            Some(value.to_string())
        };
        TransactionLogger::log_submission(
            "event_monitor",
            &monitor.name,
            &context.network,
            value_str.as_deref(),
        );

        // Build transaction with gas configuration
        let start_time = Instant::now();
        let tx_builder = GasAwareTransactionBuilder::new(
            provider.clone(),
            target_address,
            call_data.clone(),
            network_config.clone(),
        )
        .with_value(value);

        // Apply gas limit override from config
        let tx_builder = if call_config.gas_limit_multiplier > 0.0 {
            // We'll apply the multiplier after estimation, for now just use builder as-is
            tx_builder
        } else {
            tx_builder
        };

        // Check gas price against configured maximum
        if call_config.max_gas_price_gwei > 0 {
            let current_gas_price =
                provider
                    .get_gas_price()
                    .await
                    .map_err(|e| EventMonitorError::HandlerError {
                        monitor: monitor.name.clone(),
                        reason: format!("Failed to get gas price: {e}"),
                    })?;

            let max_gas_price_wei =
                crate::gas::utils::gwei_to_wei(call_config.max_gas_price_gwei as f64);
            if U256::from(current_gas_price) > max_gas_price_wei {
                return Err(EventMonitorError::HandlerError {
                    monitor: monitor.name.clone(),
                    reason: format!(
                        "Gas price {} gwei exceeds maximum {} gwei",
                        crate::gas::utils::wei_to_gwei(U256::from(current_gas_price)),
                        call_config.max_gas_price_gwei
                    ),
                });
            }
        }

        // Build and send transaction
        let tx_request = tx_builder
            .build()
            .await
            .map_err(|e| EventMonitorError::HandlerError {
                monitor: monitor.name.clone(),
                reason: format!("Failed to build transaction: {e}"),
            })?;

        let pending_tx = provider.send_transaction(tx_request).await.map_err(|e| {
            EventMonitorError::HandlerError {
                monitor: monitor.name.clone(),
                reason: format!("Failed to send transaction: {e}"),
            }
        })?;

        let tx_hash = *pending_tx.tx_hash();
        info!(
            "Transaction submitted for monitor '{}': 0x{}",
            monitor.name,
            hex::encode(tx_hash)
        );

        // Wait for confirmation
        let receipt =
            pending_tx
                .get_receipt()
                .await
                .map_err(|e| EventMonitorError::HandlerError {
                    monitor: monitor.name.clone(),
                    reason: format!("Failed to get transaction receipt: {e}"),
                })?;

        // Record metrics
        let submission_time = start_time.elapsed();
        let tx_recorder =
            TransactionMetricsRecorder::new(metrics_ctx.clone(), &network_config.transaction_type);

        if receipt.status() {
            let gas_used = receipt.gas_used;
            // TODO: Get actual gas limit from transaction
            tx_recorder.record_success(
                &receipt,
                U256::from(gas_used),
                Some(submission_time.as_secs()),
            );
            TransactionLogger::log_confirmation(tx_hash, gas_used);
            recorder.record_success(None);

            info!(
                "Contract call successful for monitor '{}': {} (gas used: {})",
                monitor.name, call.function, gas_used
            );
        } else {
            // TODO: Get actual gas limit and price from transaction
            tx_recorder.record_failure(U256::from(300000), None, "execution_reverted");
            recorder.record_failure("Transaction reverted");

            return Err(EventMonitorError::HandlerError {
                monitor: monitor.name.clone(),
                reason: format!("Transaction reverted: 0x{}", hex::encode(tx_hash)),
            });
        }

        Ok(())
    }
}

#[async_trait]
impl Handler for StoreDbHandler {
    async fn handle(
        &self,
        monitor: &EventMonitor,
        response: WebhookResponse,
        event: &ProcessedEvent,
        _context: &EventContext,
    ) -> Result<()> {
        info!(
            "Database storage handler for monitor '{}' - Phase 4 implementation pending",
            monitor.name
        );

        debug!(
            "Would store event {} from block {} with response action '{}'",
            event.event_name, event.block_number, response.action
        );

        // Phase 4: Implement database storage
        Ok(())
    }
}

#[async_trait]
impl Handler for MultiActionHandler {
    async fn handle(
        &self,
        monitor: &EventMonitor,
        response: WebhookResponse,
        event: &ProcessedEvent,
        context: &EventContext,
    ) -> Result<()> {
        info!(
            "Executing {} handlers for multi-action response on monitor '{}'",
            self.handlers.len(),
            monitor.name
        );

        for (i, handler) in self.handlers.iter().enumerate() {
            debug!("Executing handler {} of {}", i + 1, self.handlers.len());
            handler
                .handle(monitor, response.clone(), event, context)
                .await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_monitors::models::{ResponseConfig, WebhookConfig};
    use crate::event_monitors::webhook_caller::ContractCall;
    use crate::network::NetworkManager;
    use alloy::primitives::address;
    use std::collections::HashMap;

    async fn create_test_network_manager() -> Arc<NetworkManager> {
        let networks = vec![];
        Arc::new(NetworkManager::new(&networks).await.unwrap())
    }

    fn test_monitor(response_type: ResponseType) -> EventMonitor {
        EventMonitor {
            name: "test_monitor".to_string(),
            network: "ethereum-mainnet".to_string(),
            contract_address: address!("1234567890123456789012345678901234567890"),
            event_signature: "TestEvent(uint256)".to_string(),
            webhook: WebhookConfig {
                url: "https://example.com".to_string(),
                method: super::super::models::HttpMethod::Post,
                headers: HashMap::new(),
                timeout_seconds: 30,
                retry_attempts: 3,
                retry_delay_seconds: 5,
            },
            response: ResponseConfig {
                response_type,
                contract_call: None,
                validation: None,
            },
        }
    }

    fn test_response() -> WebhookResponse {
        WebhookResponse {
            action: "test_action".to_string(),
            calls: None,
            metadata: Some(serde_json::json!({"test": "data"})),
            extra: serde_json::Map::new(),
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

    #[tokio::test]
    async fn test_log_only_handler() {
        let network_manager = create_test_network_manager().await;
        let handler = ResponseHandler::new(network_manager);
        let monitor = test_monitor(ResponseType::LogOnly);
        let response = test_response();
        let event = test_event();
        let context = test_context();

        let result = handler
            .handle_response(&monitor, response, &event, &context)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handler_registration() {
        let network_manager = create_test_network_manager().await;
        let mut handler = ResponseHandler::new(network_manager);

        // Verify default handlers exist
        assert_eq!(handler.handlers.len(), 4);
        assert!(handler.handlers.contains_key(&ResponseType::LogOnly));
        assert!(handler.handlers.contains_key(&ResponseType::ContractCall));
        assert!(handler.handlers.contains_key(&ResponseType::StoreDb));
        assert!(handler.handlers.contains_key(&ResponseType::MultiAction));

        // Test registering a custom handler
        struct CustomHandler;
        #[async_trait]
        impl Handler for CustomHandler {
            async fn handle(
                &self,
                _monitor: &EventMonitor,
                _response: WebhookResponse,
                _event: &ProcessedEvent,
                _context: &EventContext,
            ) -> Result<()> {
                Ok(())
            }
        }

        handler.register_handler(ResponseType::LogOnly, Arc::new(CustomHandler));
        // Should still have 4 handlers (replaced LogOnly)
        assert_eq!(handler.handlers.len(), 4);
    }

    #[tokio::test]
    async fn test_handle_response_with_unknown_type() {
        let network_manager = create_test_network_manager().await;
        let handler = ResponseHandler::new(network_manager);
        let monitor = test_monitor(ResponseType::LogOnly);
        let response = test_response();
        let event = test_event();
        let context = test_context();

        // Clear handlers to test missing handler error
        let mut handler_mut = handler;
        handler_mut.handlers.clear();

        let result = handler_mut
            .handle_response(&monitor, response, &event, &context)
            .await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No handler found for response type"));
    }

    #[tokio::test]
    async fn test_store_db_handler() {
        let network_manager = create_test_network_manager().await;
        let handler = ResponseHandler::new(network_manager);
        let monitor = test_monitor(ResponseType::StoreDb);
        let response = test_response();
        let event = test_event();
        let context = test_context();

        // StoreDbHandler currently just logs, so it should succeed
        let result = handler
            .handle_response(&monitor, response, &event, &context)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_multi_action_handler() {
        let network_manager = create_test_network_manager().await;
        let handler = ResponseHandler::new(network_manager);
        let monitor = test_monitor(ResponseType::MultiAction);
        let response = test_response();
        let event = test_event();
        let context = test_context();

        // MultiActionHandler should execute multiple handlers
        let result = handler
            .handle_response(&monitor, response, &event, &context)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_contract_call_handler_wrong_action() {
        let network_manager = create_test_network_manager().await;
        let handler = ResponseHandler::new(network_manager);
        let monitor = test_monitor(ResponseType::ContractCall);
        let mut response = test_response();
        response.action = "wrong_action".to_string();
        let event = test_event();
        let context = test_context();

        // Should succeed but log warning
        let result = handler
            .handle_response(&monitor, response, &event, &context)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_contract_call_handler_no_calls() {
        let network_manager = create_test_network_manager().await;
        let handler = ResponseHandler::new(network_manager);
        let monitor = test_monitor(ResponseType::ContractCall);
        let mut response = test_response();
        response.action = "contract_call".to_string();
        response.calls = None;
        let event = test_event();
        let context = test_context();

        // Should fail with no calls provided
        let result = handler
            .handle_response(&monitor, response, &event, &context)
            .await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No contract calls provided"));
    }

    #[tokio::test]
    async fn test_handle_response_metrics() {
        let network_manager = create_test_network_manager().await;
        let handler = ResponseHandler::new(network_manager);
        let monitor = test_monitor(ResponseType::LogOnly);
        let response = test_response();
        let event = test_event();
        let context = test_context();

        // Test that metrics are recorded (should succeed)
        let result = handler
            .handle_response(&monitor, response, &event, &context)
            .await;
        assert!(result.is_ok());
    }

    fn test_contract_call() -> ContractCall {
        ContractCall {
            target: "0x1234567890123456789012345678901234567890".to_string(),
            function: "transfer(address,uint256)".to_string(),
            params: vec![
                serde_json::json!("0x2345678901234567890123456789012345678901"),
                serde_json::json!("1000000"),
            ],
            value: "0".to_string(),
        }
    }

    #[test]
    fn test_contract_call_serialization() {
        let call = test_contract_call();
        let json = serde_json::to_string(&call).unwrap();
        assert!(json.contains("transfer(address,uint256)"));
        assert!(json.contains("0x1234567890123456789012345678901234567890"));
    }
}
