use crate::scheduled_tasks::models::CheckCondition;
use alloy::{
    dyn_abi::{DynSolValue, FunctionExt, JsonAbiExt},
    json_abi::{Function, Param, StateMutability},
    network::Network,
    primitives::{Address, U256},
    providers::Provider,
    transports::Transport,
};
use anyhow::{anyhow, Result};
use std::sync::Arc;
use tracing::{debug, error, info, trace};

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
        debug!("=== Checking condition ===");
        debug!("Condition type: {:?}", condition);

        match condition {
            CheckCondition::Property {
                contract_address,
                property,
                expected_value,
            } => {
                debug!(
                    "Checking property '{}' on contract {}",
                    property, contract_address
                );
                debug!("Expected value: {:?}", expected_value);

                Self::check_property(provider, contract_address, property, expected_value)
                    .await
                    .map_err(|e| {
                        error!("Property check failed: {:?}", e);
                        e
                    })
            }
            CheckCondition::Function {
                contract_address,
                function,
                expected_value,
            } => {
                debug!(
                    "Checking function '{}' on contract {}",
                    function, contract_address
                );
                debug!("Expected value: {:?}", expected_value);

                Self::check_function(provider, contract_address, function, expected_value)
                    .await
                    .map_err(|e| {
                        error!("Function check failed: {:?}", e);
                        e
                    })
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
        debug!("Parsing contract address: {}", contract_address);
        let address = contract_address.parse::<Address>()?;
        debug!("Parsed address successfully: {:?}", address);

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
        debug!("Created function definition for property '{}'", property);

        // Encode the function call
        let encoded_call = function.abi_encode_input(&[])?;
        debug!("Encoded property call: 0x{}", hex::encode(&encoded_call));

        // Build transaction request
        let tx_request = alloy::rpc::types::TransactionRequest::default()
            .to(address)
            .input(encoded_call.into());
        trace!("Built transaction request: {:?}", tx_request);

        // Convert to network-specific type
        let network_tx = N::TransactionRequest::from(tx_request);

        // Make the call
        debug!("Making eth_call for property '{}'...", property);
        let result = provider.call(&network_tx).await.map_err(|e| {
            error!("eth_call failed for property '{}': {:?}", property, e);
            e
        })?;
        debug!("eth_call succeeded, result: 0x{}", hex::encode(&result));

        // Decode the result
        let decoded = function.abi_decode_output(&result, true).map_err(|e| {
            error!(
                "Failed to decode result for property '{}': {:?}",
                property, e
            );
            e
        })?;
        debug!("Decoded result: {:?}", decoded);

        // Get the boolean value
        let result_bool = match decoded.first() {
            Some(DynSolValue::Bool(b)) => *b,
            _ => {
                error!(
                    "Expected boolean return value but got: {:?}",
                    decoded.first()
                );
                return Err(anyhow!("Expected boolean return value"));
            }
        };

        // Compare with expected value
        let expected_bool = expected_value
            .as_bool()
            .ok_or_else(|| anyhow!("Expected value must be a boolean for property check"))?;

        debug!(
            "Property '{}' returned {}, expected {} => match: {}",
            property,
            result_bool,
            expected_bool,
            result_bool == expected_bool
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
        debug!(
            "Checking function '{}' on contract {}",
            function, contract_address
        );
        debug!("Parsing contract address: {}", contract_address);
        let address = contract_address.parse::<Address>().map_err(|e| {
            error!(
                "Failed to parse contract address '{}': {:?}",
                contract_address, e
            );
            e
        })?;
        debug!("Parsed address successfully: {:?}", address);

        // Parse function signature to determine return type
        let (func_name, return_type) = Self::parse_function_signature(function)?;
        debug!(
            "Parsed function name: '{}', return type: '{}'",
            func_name, return_type
        );

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
        debug!("Encoded function call: 0x{}", hex::encode(&encoded_call));

        // Build transaction request
        let tx_request = alloy::rpc::types::TransactionRequest::default()
            .to(address)
            .input(encoded_call.into());

        // Convert to network-specific type
        let network_tx = N::TransactionRequest::from(tx_request);

        // Make the call
        debug!("Making eth_call to contract...");
        let result = provider.call(&network_tx).await.map_err(|e| {
            debug!("eth_call failed: {}", e);
            e
        })?;
        debug!("eth_call succeeded, decoding result...");

        // Decode the result
        debug!("Decoding result for function '{}'...", func_name);
        let decoded = func_def.abi_decode_output(&result, true).map_err(|e| {
            error!(
                "Failed to decode result for function '{}': {:?}",
                func_name, e
            );
            e
        })?;
        debug!("Decoded result: {:?}", decoded);

        // Handle different return types
        let result = match (return_type.as_str(), decoded.first()) {
            ("bool", Some(DynSolValue::Bool(b))) => {
                let expected_bool = expected_value
                    .as_bool()
                    .ok_or_else(|| anyhow!("Expected value must be a boolean"))?;
                debug!(
                    "Function '{}' returned bool: {}, expected: {}",
                    func_name, b, expected_bool
                );
                Ok(*b == expected_bool)
            }
            ("uint256", Some(DynSolValue::Uint(val, _))) => {
                let expected_str = expected_value
                    .as_str()
                    .ok_or_else(|| anyhow!("Expected value must be a string for uint256"))?;
                let expected_u256 = U256::from_str_radix(expected_str, 10)?;
                info!(
                    "Function '{}' returned uint256: {}, expected: {}",
                    func_name, val, expected_u256
                );
                Ok(val == &expected_u256)
            }
            _ => {
                error!(
                    "Unsupported or mismatched return type: {}. Got: {:?}",
                    return_type,
                    decoded.first()
                );
                Err(anyhow!(
                    "Unsupported or mismatched return type: {}",
                    return_type
                ))
            }
        };

        match &result {
            Ok(matches) => debug!("Condition check result for '{}': {}", func_name, matches),
            Err(e) => error!("Condition check failed for '{}': {:?}", func_name, e),
        }

        result
    }

    fn parse_function_signature(signature: &str) -> Result<(String, String)> {
        debug!("Parsing function signature: '{}'", signature);
        let signature = signature.trim();

        // Check if it has a return type specification like "funcName() (returnType)"
        if let Some(paren_pos) = signature.find("()") {
            let func_name = &signature[..paren_pos];
            let remaining = &signature[paren_pos + 2..].trim();

            let return_type = if remaining.starts_with('(') && remaining.ends_with(')') {
                // Extract return type from "(returnType)"
                remaining[1..remaining.len() - 1].to_string()
            } else if remaining.is_empty() {
                // No return type specified, assume bool
                "bool".to_string()
            } else {
                return Err(anyhow!(
                    "Invalid function signature format: {}. Expected 'funcName()' or 'funcName() (returnType)'",
                    signature
                ));
            };

            debug!(
                "Parsed function: name='{}', return_type='{}'",
                func_name, return_type
            );
            Ok((func_name.to_string(), return_type))
        } else {
            Err(anyhow!(
                "Function signature must contain '()': {}",
                signature
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_function_signature() {
        // Test simple function without return type
        let (name, ret_type) =
            ConditionChecker::parse_function_signature("isReady()").expect("Failed to parse");
        assert_eq!(name, "isReady");
        assert_eq!(ret_type, "bool");

        // Test function with bool return type
        let (name, ret_type) =
            ConditionChecker::parse_function_signature("canDistributeRewards() (bool)")
                .expect("Failed to parse");
        assert_eq!(name, "canDistributeRewards");
        assert_eq!(ret_type, "bool");

        // Test function with uint256 return type
        let (name, ret_type) = ConditionChecker::parse_function_signature("getBalance() (uint256)")
            .expect("Failed to parse");
        assert_eq!(name, "getBalance");
        assert_eq!(ret_type, "uint256");
    }

    #[test]
    fn test_parse_function_signature_invalid() {
        assert!(ConditionChecker::parse_function_signature("isReady").is_err());
        assert!(ConditionChecker::parse_function_signature("isReady(uint256)").is_err());
    }
}
