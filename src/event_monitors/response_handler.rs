//! Response handling framework for webhook responses

use super::listener::{EventContext, ProcessedEvent};
use super::models::{EventMonitor, ResponseType};
use super::webhook_caller::WebhookResponse;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Handles webhook responses based on configured response type
pub struct ResponseHandler {
    handlers: HashMap<ResponseType, Arc<dyn Handler>>,
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

/// Handler for contract calls (placeholder for Phase 2)
pub struct ContractCallHandler;

/// Handler for database storage (placeholder for Phase 4)
pub struct StoreDbHandler;

/// Handler for multiple actions
pub struct MultiActionHandler {
    handlers: Vec<Arc<dyn Handler>>,
}

impl ResponseHandler {
    /// Create a new response handler with default handlers
    pub fn new() -> Self {
        let mut handlers: HashMap<ResponseType, Arc<dyn Handler>> = HashMap::new();

        handlers.insert(ResponseType::LogOnly, Arc::new(LogOnlyHandler));
        handlers.insert(ResponseType::ContractCall, Arc::new(ContractCallHandler));
        handlers.insert(ResponseType::StoreDb, Arc::new(StoreDbHandler));
        handlers.insert(
            ResponseType::MultiAction,
            Arc::new(MultiActionHandler {
                handlers: vec![Arc::new(LogOnlyHandler), Arc::new(StoreDbHandler)],
            }),
        );

        Self { handlers }
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

        let handler = self.handlers.get(response_type).context(format!(
            "No handler found for response type {response_type:?}"
        ))?;

        handler.handle(monitor, response, event, context).await
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
        _context: &EventContext,
    ) -> Result<()> {
        info!(
            "Contract call handler for monitor '{}' - Phase 2 implementation pending",
            monitor.name
        );

        if response.action != "contract_call" {
            warn!(
                "Expected 'contract_call' action but got '{}' for monitor '{}'",
                response.action, monitor.name
            );
            return Ok(());
        }

        if let Some(calls) = response.calls {
            debug!(
                "Would execute {} contract calls for event {} at block {}",
                calls.len(),
                event.event_name,
                event.block_number
            );
            // Phase 2: Implement actual contract execution
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

impl Default for ResponseHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_monitors::models::{ResponseConfig, WebhookConfig};
    use alloy::primitives::address;
    use std::collections::HashMap;

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
        let handler = ResponseHandler::new();
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
        let handler = ResponseHandler::new();

        // Verify default handlers exist
        assert_eq!(handler.handlers.len(), 4);
        assert!(handler.handlers.contains_key(&ResponseType::LogOnly));
        assert!(handler.handlers.contains_key(&ResponseType::ContractCall));
        assert!(handler.handlers.contains_key(&ResponseType::StoreDb));
        assert!(handler.handlers.contains_key(&ResponseType::MultiAction));
    }
}
