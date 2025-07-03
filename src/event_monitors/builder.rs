//! Builder pattern for event monitor configuration

use super::error::{EventMonitorError, Result};
use super::models::{
    ContractCallConfig, EventMonitor, HttpMethod, ResponseConfig, ResponseType, ValidationConfig,
    WebhookConfig,
};
use alloy::primitives::Address;
use std::collections::HashMap;

/// Builder for creating EventMonitor configurations
pub struct EventMonitorBuilder {
    name: Option<String>,
    network: Option<String>,
    contract_address: Option<Address>,
    event_signature: Option<String>,
    webhook_url: Option<String>,
    webhook_method: HttpMethod,
    webhook_headers: HashMap<String, String>,
    webhook_timeout_seconds: u64,
    webhook_retry_attempts: u8,
    webhook_retry_delay_seconds: u64,
    response_type: ResponseType,
    contract_call_config: Option<ContractCallConfig>,
    validation_config: Option<ValidationConfig>,
}

impl Default for EventMonitorBuilder {
    fn default() -> Self {
        Self {
            name: None,
            network: None,
            contract_address: None,
            event_signature: None,
            webhook_url: None,
            webhook_method: HttpMethod::Post,
            webhook_headers: HashMap::new(),
            webhook_timeout_seconds: 30,
            webhook_retry_attempts: 3,
            webhook_retry_delay_seconds: 5,
            response_type: ResponseType::LogOnly,
            contract_call_config: None,
            validation_config: None,
        }
    }
}

impl EventMonitorBuilder {
    /// Create a new event monitor builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the monitor name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the network
    pub fn network(mut self, network: impl Into<String>) -> Self {
        self.network = Some(network.into());
        self
    }

    /// Set the contract address
    pub fn contract_address(mut self, address: Address) -> Self {
        self.contract_address = Some(address);
        self
    }

    /// Set the event signature
    pub fn event_signature(mut self, signature: impl Into<String>) -> Self {
        self.event_signature = Some(signature.into());
        self
    }

    /// Set the webhook URL
    pub fn webhook_url(mut self, url: impl Into<String>) -> Self {
        self.webhook_url = Some(url.into());
        self
    }

    /// Set the webhook HTTP method
    pub fn webhook_method(mut self, method: HttpMethod) -> Self {
        self.webhook_method = method;
        self
    }

    /// Add a webhook header
    pub fn webhook_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.webhook_headers.insert(key.into(), value.into());
        self
    }

    /// Set webhook headers
    pub fn webhook_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.webhook_headers = headers;
        self
    }

    /// Set webhook timeout
    pub fn webhook_timeout(mut self, seconds: u64) -> Self {
        self.webhook_timeout_seconds = seconds;
        self
    }

    /// Set webhook retry attempts
    pub fn webhook_retries(mut self, attempts: u8) -> Self {
        self.webhook_retry_attempts = attempts;
        self
    }

    /// Set webhook retry delay
    pub fn webhook_retry_delay(mut self, seconds: u64) -> Self {
        self.webhook_retry_delay_seconds = seconds;
        self
    }

    /// Set response type
    pub fn response_type(mut self, response_type: ResponseType) -> Self {
        self.response_type = response_type;
        self
    }

    /// Set contract call configuration
    pub fn contract_call(mut self, target: impl Into<String>, max_gas_price_gwei: u64) -> Self {
        self.contract_call_config = Some(ContractCallConfig {
            target_contract: target.into(),
            max_gas_price_gwei,
            gas_limit_multiplier: 1.2,
            value_wei: 0,
        });
        self.response_type = ResponseType::ContractCall;
        self
    }

    /// Set validation configuration
    pub fn require_signature(mut self, allowed_signers: Vec<Address>) -> Self {
        self.validation_config = Some(ValidationConfig {
            require_signature: true,
            allowed_signers,
            max_response_age_seconds: 300,
        });
        self
    }

    /// Set validation with custom age
    pub fn validation(
        mut self,
        require_signature: bool,
        allowed_signers: Vec<Address>,
        max_age_seconds: u64,
    ) -> Self {
        self.validation_config = Some(ValidationConfig {
            require_signature,
            allowed_signers,
            max_response_age_seconds: max_age_seconds,
        });
        self
    }

    /// Build the EventMonitor
    pub fn build(self) -> Result<EventMonitor> {
        let name = self.name.ok_or_else(|| EventMonitorError::ConfigError {
            monitor: "<unnamed>".to_string(),
            reason: "Monitor name is required".to_string(),
        })?;

        let network = self.network.ok_or_else(|| EventMonitorError::ConfigError {
            monitor: name.clone(),
            reason: "Network is required".to_string(),
        })?;

        let contract_address =
            self.contract_address
                .ok_or_else(|| EventMonitorError::ConfigError {
                    monitor: name.clone(),
                    reason: "Contract address is required".to_string(),
                })?;

        let event_signature =
            self.event_signature
                .ok_or_else(|| EventMonitorError::ConfigError {
                    monitor: name.clone(),
                    reason: "Event signature is required".to_string(),
                })?;

        let webhook_url = self
            .webhook_url
            .ok_or_else(|| EventMonitorError::ConfigError {
                monitor: name.clone(),
                reason: "Webhook URL is required".to_string(),
            })?;

        let webhook = WebhookConfig {
            url: webhook_url,
            method: self.webhook_method,
            headers: self.webhook_headers,
            timeout_seconds: self.webhook_timeout_seconds,
            retry_attempts: self.webhook_retry_attempts,
            retry_delay_seconds: self.webhook_retry_delay_seconds,
        };

        let response = ResponseConfig {
            response_type: self.response_type,
            contract_call: self.contract_call_config,
            validation: self.validation_config,
        };

        let monitor = EventMonitor {
            name,
            network,
            contract_address,
            event_signature,
            webhook,
            response,
        };

        // Validate the built monitor
        monitor
            .validate()
            .map_err(|reason| EventMonitorError::ConfigError {
                monitor: monitor.name.clone(),
                reason,
            })?;

        Ok(monitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::address;

    #[test]
    fn test_basic_builder() {
        let monitor = EventMonitorBuilder::new()
            .name("test_monitor")
            .network("ethereum-mainnet")
            .contract_address(address!("1234567890123456789012345678901234567890"))
            .event_signature("Transfer(address,address,uint256)")
            .webhook_url("https://example.com/webhook")
            .build()
            .unwrap();

        assert_eq!(monitor.name, "test_monitor");
        assert_eq!(monitor.network, "ethereum-mainnet");
        assert_eq!(monitor.webhook.method, HttpMethod::Post);
        assert_eq!(monitor.webhook.retry_attempts, 3);
    }

    #[test]
    fn test_builder_with_contract_call() {
        let monitor = EventMonitorBuilder::new()
            .name("test_monitor")
            .network("ethereum-mainnet")
            .contract_address(address!("1234567890123456789012345678901234567890"))
            .event_signature("Transfer(address,address,uint256)")
            .webhook_url("https://example.com/webhook")
            .contract_call("0xabcd", 100)
            .build()
            .unwrap();

        assert_eq!(monitor.response.response_type, ResponseType::ContractCall);
        assert!(monitor.response.contract_call.is_some());
    }

    #[test]
    fn test_builder_with_validation() {
        let signers = vec![address!("1234567890123456789012345678901234567890")];

        let monitor = EventMonitorBuilder::new()
            .name("test_monitor")
            .network("ethereum-mainnet")
            .contract_address(address!("1234567890123456789012345678901234567890"))
            .event_signature("Transfer(address,address,uint256)")
            .webhook_url("https://example.com/webhook")
            .require_signature(signers.clone())
            .build()
            .unwrap();

        let validation = monitor.response.validation.unwrap();
        assert!(validation.require_signature);
        assert_eq!(validation.allowed_signers, signers);
    }

    #[test]
    fn test_builder_missing_required() {
        let result = EventMonitorBuilder::new().build();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("name is required"));
    }

    #[test]
    fn test_builder_with_headers() {
        let monitor = EventMonitorBuilder::new()
            .name("test_monitor")
            .network("ethereum-mainnet")
            .contract_address(address!("1234567890123456789012345678901234567890"))
            .event_signature("Transfer(address,address,uint256)")
            .webhook_url("https://example.com/webhook")
            .webhook_header("Authorization", "Bearer token")
            .webhook_header("X-Custom", "value")
            .build()
            .unwrap();

        assert_eq!(monitor.webhook.headers.len(), 2);
        assert_eq!(
            monitor.webhook.headers.get("Authorization").unwrap(),
            "Bearer token"
        );
    }
}
