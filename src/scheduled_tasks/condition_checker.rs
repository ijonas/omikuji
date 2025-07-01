use crate::scheduled_tasks::models::CheckCondition;
use alloy::{
    dyn_abi::{DynSolValue, JsonAbiExt, FunctionExt},
    json_abi::{Function, Param, StateMutability},
    network::Network,
    primitives::{Address, U256},
    providers::Provider,
    transports::Transport,
};
use anyhow::{anyhow, Result};
use std::sync::Arc;
use tracing::debug;

pub struct ConditionChecker;

impl ConditionChecker {
    pub async fn check_condition<T, N, P>(
        provider: Arc<P>,
        condition: &CheckCondition,
    ) -> Result<bool>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
        N::TransactionRequest: Default + From<alloy::rpc::types::TransactionRequest>,
    {
        match condition {
            CheckCondition::Property {
                contract_address,
                property,
                expected_value,
            } => {
                Self::check_property(
                    provider,
                    contract_address,
                    property,
                    expected_value,
                )
                .await
            }
            CheckCondition::Function {
                contract_address,
                function,
                expected_value,
            } => {
                Self::check_function(
                    provider,
                    contract_address,
                    function,
                    expected_value,
                )
                .await
            }
        }
    }

    async fn check_property<T, N, P>(
        provider: Arc<P>,
        contract_address: &str,
        property: &str,
        expected_value: &serde_json::Value,
    ) -> Result<bool>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
        N::TransactionRequest: Default + From<alloy::rpc::types::TransactionRequest>,
    {
        let address = contract_address.parse::<Address>()?;
        
        // Create function selector for property getter
        let function = Function {
            name: property.to_string(),
            inputs: vec![],
            outputs: vec![Param {
                ty: "bool".to_string(),
                name: "".to_string(),
                components: vec![],
                internal_type: None,
            }],
            state_mutability: StateMutability::View,
        };

        // Encode the function call
        let encoded_call = function.abi_encode_input(&[])?;

        // Build transaction request
        let tx_request = alloy::rpc::types::TransactionRequest::default()
            .to(address)
            .input(encoded_call.into());
        
        // Convert to network-specific type
        let network_tx = N::TransactionRequest::from(tx_request);

        // Make the call
        let result = provider.call(&network_tx).await?;

        // Decode the result  
        let decoded = function.abi_decode_output(&result, true)?;
        
        // Get the boolean value
        let result_bool = match decoded.first() {
            Some(DynSolValue::Bool(b)) => *b,
            _ => return Err(anyhow!("Expected boolean return value")),
        };

        // Compare with expected value
        let expected_bool = expected_value
            .as_bool()
            .ok_or_else(|| anyhow!("Expected value must be a boolean for property check"))?;

        debug!(
            "Property '{}' returned {}, expected {}",
            property, result_bool, expected_bool
        );

        Ok(result_bool == expected_bool)
    }

    async fn check_function<T, N, P>(
        provider: Arc<P>,
        contract_address: &str,
        function: &str,
        expected_value: &serde_json::Value,
    ) -> Result<bool>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N>,
        N::TransactionRequest: Default + From<alloy::rpc::types::TransactionRequest>,
    {
        let address = contract_address.parse::<Address>()?;

        // Parse function signature to determine return type
        let (func_name, return_type) = Self::parse_function_signature(function)?;

        // Create function definition based on signature
        let func_def = Function {
            name: func_name.clone(),
            inputs: vec![], // Parameterless function
            outputs: vec![Param {
                ty: return_type.clone(),
                name: "".to_string(),
                components: vec![],
                internal_type: None,
            }],
            state_mutability: StateMutability::View,
        };

        // Encode the function call
        let encoded_call = func_def.abi_encode_input(&[])?;

        // Build transaction request
        let tx_request = alloy::rpc::types::TransactionRequest::default()
            .to(address)
            .input(encoded_call.into());
        
        // Convert to network-specific type
        let network_tx = N::TransactionRequest::from(tx_request);

        // Make the call
        let result = provider.call(&network_tx).await?;

        // Decode the result
        let decoded = func_def.abi_decode_output(&result, true)?;
        
        // Handle different return types
        match (return_type.as_str(), decoded.first()) {
            ("bool", Some(DynSolValue::Bool(b))) => {
                let expected_bool = expected_value
                    .as_bool()
                    .ok_or_else(|| anyhow!("Expected value must be a boolean"))?;
                Ok(*b == expected_bool)
            }
            ("uint256", Some(DynSolValue::Uint(val, _))) => {
                let expected_str = expected_value
                    .as_str()
                    .ok_or_else(|| anyhow!("Expected value must be a string for uint256"))?;
                let expected_u256 = U256::from_str_radix(expected_str, 10)?;
                Ok(val == &expected_u256)
            }
            _ => Err(anyhow!("Unsupported or mismatched return type: {}", return_type)),
        }
    }

    fn parse_function_signature(signature: &str) -> Result<(String, String)> {
        // Remove parentheses and parse
        let signature = signature.trim();
        if !signature.ends_with("()") {
            return Err(anyhow!(
                "Function must be parameterless and end with '()': {}",
                signature
            ));
        }

        let func_name = signature.trim_end_matches("()");
        
        // For now, assume boolean return type unless specified
        // In a full implementation, we might parse return type from signature
        Ok((func_name.to_string(), "bool".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_function_signature() {
        let (name, ret_type) = ConditionChecker::parse_function_signature("isReady()")
            .expect("Failed to parse");
        assert_eq!(name, "isReady");
        assert_eq!(ret_type, "bool");
    }

    #[test]
    fn test_parse_function_signature_invalid() {
        assert!(ConditionChecker::parse_function_signature("isReady").is_err());
        assert!(ConditionChecker::parse_function_signature("isReady(uint256)").is_err());
    }
}