//! HTTP webhook client with retry logic

use super::error::{EventMonitorError, Result};
use super::listener::{EventContext, ProcessedEvent};
use super::metrics::{EventMonitorMetricsContext, WebhookRetryMetricsRecorder};
use super::models::{HttpMethod, WebhookConfig};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/// HTTP client for calling webhooks
pub struct WebhookCaller {
    client: Client,
}

/// Webhook request payload
#[derive(Debug, Serialize)]
pub struct WebhookPayload {
    pub monitor_name: String,
    pub event: EventData,
    pub context: ContextData,
}

/// Event data for webhook payload
#[derive(Debug, Serialize)]
pub struct EventData {
    pub name: String,
    pub contract_address: String,
    pub transaction_hash: String,
    pub block_number: u64,
    pub log_index: u64,
    pub removed: bool,
    pub topics: Vec<String>,
    pub data: String,
    pub decoded_args: serde_json::Value,
}

/// Context data for webhook payload
#[derive(Debug, Serialize)]
pub struct ContextData {
    pub network: String,
    pub timestamp: String,
    pub omikuji_version: String,
}

/// Webhook response
#[derive(Debug, Clone, Deserialize)]
pub struct WebhookResponse {
    pub action: String,
    pub calls: Option<Vec<ContractCall>>,
    pub metadata: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Contract call data from webhook response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractCall {
    /// Target contract address
    pub target: String,
    /// Function signature (e.g., "transfer(address,uint256)")
    pub function: String,
    /// Function parameters as JSON values
    pub params: Vec<serde_json::Value>,
    /// ETH value to send in wei (optional, defaults to 0)
    #[serde(default)]
    pub value: String,
}

impl WebhookCaller {
    /// Create a new webhook caller
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent(format!("Omikuji/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }

    /// Call a webhook with retry logic
    pub async fn call_webhook(
        &self,
        config: &WebhookConfig,
        event: &ProcessedEvent,
        context: &EventContext,
    ) -> Result<WebhookResponse> {
        let metrics_ctx =
            EventMonitorMetricsContext::new(event.monitor_name.clone(), context.network.clone());
        let retry_recorder = WebhookRetryMetricsRecorder::new(event.monitor_name.clone());
        let payload = self.create_payload(event, context);

        debug!(
            "Calling webhook '{}' for event from monitor '{}'",
            config.url, event.monitor_name
        );

        let mut last_error = None;
        let mut attempt = 0;

        while attempt <= config.retry_attempts {
            let start_time = Instant::now();
            match self.make_request(config, &payload).await {
                Ok(response) => {
                    let duration = start_time.elapsed().as_secs_f64();
                    metrics_ctx.webhook_response_time(duration);
                    metrics_ctx.webhook_call(true);

                    if attempt > 0 {
                        retry_recorder.record_result(true, (attempt + 1) as u32);
                    }

                    info!(
                        "Webhook call successful for monitor '{}' (attempt {}/{})",
                        event.monitor_name,
                        attempt + 1,
                        config.retry_attempts + 1
                    );
                    return Ok(response);
                }
                Err(e) => {
                    attempt += 1;
                    last_error = Some(e);

                    if attempt > 1 {
                        retry_recorder.record_attempt(attempt as u32, "http_error");
                    }

                    if attempt <= config.retry_attempts {
                        warn!(
                            "Webhook call failed for monitor '{}' (attempt {}/{}): {}. Retrying in {}s...",
                            event.monitor_name,
                            attempt,
                            config.retry_attempts + 1,
                            last_error.as_ref().unwrap(),
                            config.retry_delay_seconds
                        );

                        // Exponential backoff: delay * 2^(attempt-1)
                        let delay = Duration::from_secs(
                            config.retry_delay_seconds * 2u64.pow((attempt - 1).min(5) as u32),
                        );
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        metrics_ctx.webhook_call(false);
        retry_recorder.record_result(false, (config.retry_attempts + 1) as u32);

        error!(
            "Webhook call failed for monitor '{}' after {} attempts",
            event.monitor_name,
            config.retry_attempts + 1
        );

        Err(EventMonitorError::WebhookError {
            monitor: event.monitor_name.clone(),
            attempts: config.retry_attempts + 1,
            reason: last_error.unwrap().to_string(),
        })
    }

    /// Create webhook payload from event data
    fn create_payload(&self, event: &ProcessedEvent, context: &EventContext) -> WebhookPayload {
        WebhookPayload {
            monitor_name: event.monitor_name.clone(),
            event: EventData {
                name: event.event_name.clone(),
                contract_address: format!("0x{}", hex::encode(event.contract_address)),
                transaction_hash: event.transaction_hash.clone(),
                block_number: event.block_number,
                log_index: event.log_index,
                removed: event.removed,
                topics: event.topics.clone(),
                data: event.data.clone(),
                decoded_args: event.decoded_args.clone(),
            },
            context: ContextData {
                network: context.network.clone(),
                timestamp: context.timestamp.to_rfc3339(),
                omikuji_version: context.omikuji_version.clone(),
            },
        }
    }

    /// Make a single webhook request
    async fn make_request(
        &self,
        config: &WebhookConfig,
        payload: &WebhookPayload,
    ) -> Result<WebhookResponse> {
        let mut request = match config.method {
            HttpMethod::Get => self.client.get(&config.url),
            HttpMethod::Post => self.client.post(&config.url),
            HttpMethod::Put => self.client.put(&config.url),
            HttpMethod::Patch => self.client.patch(&config.url),
            HttpMethod::Delete => self.client.delete(&config.url),
        };

        // Add headers
        for (key, value) in &config.headers {
            request = request.header(key, value);
        }

        // Set timeout
        request = request.timeout(Duration::from_secs(config.timeout_seconds));

        // Add JSON body for methods that support it
        if matches!(
            config.method,
            HttpMethod::Post | HttpMethod::Put | HttpMethod::Patch
        ) {
            request = request.json(payload);
        }

        // Send request
        let response = request.send().await.map_err(EventMonitorError::HttpError)?;

        // Check status
        let status = response.status();
        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error body".to_string());

            return Err(EventMonitorError::Other(format!(
                "Webhook returned error status {status}: {error_body}"
            )));
        }

        // Parse response
        let response_text = response
            .text()
            .await
            .map_err(EventMonitorError::HttpError)?;

        serde_json::from_str(&response_text).map_err(EventMonitorError::JsonError)
    }
}

impl Default for WebhookCaller {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::address;
    use std::collections::HashMap;

    fn test_webhook_config() -> WebhookConfig {
        WebhookConfig {
            url: "https://example.com/webhook".to_string(),
            method: HttpMethod::Post,
            headers: HashMap::new(),
            timeout_seconds: 30,
            retry_attempts: 3,
            retry_delay_seconds: 5,
        }
    }

    fn test_event() -> ProcessedEvent {
        ProcessedEvent {
            monitor_name: "test_monitor".to_string(),
            event_name: "Transfer".to_string(),
            contract_address: address!("1234567890123456789012345678901234567890"),
            transaction_hash: "0xabcd".to_string(),
            block_number: 12345,
            log_index: 0,
            removed: false,
            topics: vec!["0x1234".to_string()],
            data: "0x5678".to_string(),
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
    fn test_create_payload() {
        let caller = WebhookCaller::new();
        let event = test_event();
        let context = test_context();

        let payload = caller.create_payload(&event, &context);

        assert_eq!(payload.monitor_name, "test_monitor");
        assert_eq!(payload.event.name, "Transfer");
        assert_eq!(payload.context.network, "ethereum-mainnet");
    }

    #[tokio::test]
    async fn test_webhook_caller_creation() {
        let _caller = WebhookCaller::new();
        // Just verify it can be created without panicking
        assert!(true);
    }

    #[test]
    fn test_webhook_payload_serialization() {
        let caller = WebhookCaller::new();
        let event = test_event();
        let context = test_context();
        let payload = caller.create_payload(&event, &context);

        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("test_monitor"));
        assert!(json.contains("Transfer"));
        assert!(json.contains("ethereum-mainnet"));
        assert!(json.contains("0x1234567890123456789012345678901234567890"));
    }

    #[test]
    fn test_webhook_response_deserialization() {
        // Test basic response
        let json = r#"{"action": "log_only"}"#;
        let response: WebhookResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.action, "log_only");
        assert!(response.calls.is_none());

        // Test response with contract calls
        let json = r#"{
            "action": "contract_call",
            "calls": [{
                "target": "0x1234567890123456789012345678901234567890",
                "function": "transfer(address,uint256)",
                "params": ["0x2345678901234567890123456789012345678901", "1000000"],
                "value": "0"
            }]
        }"#;
        let response: WebhookResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.action, "contract_call");
        assert!(response.calls.is_some());
        assert_eq!(response.calls.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_webhook_config_validation_errors() {
        let mut config = test_webhook_config();

        // Test invalid timeout
        config.timeout_seconds = 0;
        assert!(config.validate().is_err());
        assert!(config
            .validate()
            .unwrap_err()
            .contains("timeout must be greater than 0"));

        // Test invalid retry delay
        config.timeout_seconds = 30;
        config.retry_attempts = 3;
        config.retry_delay_seconds = 0;
        assert!(config.validate().is_err());
        assert!(config
            .validate()
            .unwrap_err()
            .contains("Retry delay must be greater than 0"));
    }

    #[test]
    fn test_http_method_serialization() {
        assert_eq!(serde_json::to_string(&HttpMethod::Get).unwrap(), "\"GET\"");
        assert_eq!(
            serde_json::to_string(&HttpMethod::Post).unwrap(),
            "\"POST\""
        );
        assert_eq!(serde_json::to_string(&HttpMethod::Put).unwrap(), "\"PUT\"");
        assert_eq!(
            serde_json::to_string(&HttpMethod::Delete).unwrap(),
            "\"DELETE\""
        );
    }

    #[tokio::test]
    async fn test_webhook_caller_default() {
        let caller1 = WebhookCaller::new();
        let caller2 = WebhookCaller::default();

        // Both should create valid instances
        let event = test_event();
        let context = test_context();
        let payload1 = caller1.create_payload(&event, &context);
        let payload2 = caller2.create_payload(&event, &context);

        assert_eq!(payload1.event.name, payload2.event.name);
    }

    #[test]
    fn test_contract_call_structure() {
        let call = ContractCall {
            target: "0x1234567890123456789012345678901234567890".to_string(),
            function: "approve(address,uint256)".to_string(),
            params: vec![
                serde_json::json!("0x2345678901234567890123456789012345678901"),
                serde_json::json!("1000000000000000000"),
            ],
            value: "0".to_string(),
        };

        let json = serde_json::to_string(&call).unwrap();
        let deserialized: ContractCall = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.target, call.target);
        assert_eq!(deserialized.function, call.function);
        assert_eq!(deserialized.params.len(), 2);
        assert_eq!(deserialized.value, "0");
    }

    #[test]
    fn test_webhook_response_with_metadata() {
        let json = r#"{
            "action": "log_only",
            "metadata": {
                "processed": true,
                "timestamp": 1234567890,
                "custom_field": "test_value"
            }
        }"#;

        let response: WebhookResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.action, "log_only");
        assert!(response.metadata.is_some());

        let metadata = response.metadata.unwrap();
        assert_eq!(metadata["processed"], true);
        assert_eq!(metadata["timestamp"], 1234567890);
        assert_eq!(metadata["custom_field"], "test_value");
    }

    #[test]
    fn test_webhook_response_clone() {
        let mut response = WebhookResponse {
            action: "test_action".to_string(),
            calls: Some(vec![ContractCall {
                target: "0x123".to_string(),
                function: "test()".to_string(),
                params: vec![],
                value: "0".to_string(),
            }]),
            metadata: Some(serde_json::json!({"key": "value"})),
            extra: serde_json::Map::new(),
        };
        response
            .extra
            .insert("extra_key".to_string(), serde_json::json!("extra_value"));

        let cloned = response.clone();
        assert_eq!(cloned.action, response.action);
        assert_eq!(cloned.calls.as_ref().unwrap().len(), 1);
        assert_eq!(cloned.metadata, response.metadata);
        assert_eq!(cloned.extra["extra_key"], "extra_value");
    }
}
