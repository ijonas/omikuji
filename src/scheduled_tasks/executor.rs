use crate::scheduled_tasks::models::{GasConfig, Parameter, TargetFunction};
use alloy::{
    dyn_abi::{DynSolValue, JsonAbiExt},
    json_abi::{Function, JsonAbi, Param, StateMutability},
    network::{Network, TransactionBuilder, ReceiptResponse},
    primitives::{Address, U256},
    providers::Provider,
    transports::Transport,
};
use anyhow::{anyhow, Context, Result};
use std::sync::Arc;
use tracing::{debug, info, trace};

pub struct FunctionExecutor<T, N, P>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    provider: Arc<P>,
    _phantom_t: std::marker::PhantomData<T>,
    _phantom_n: std::marker::PhantomData<N>,
}

impl<T, N, P> FunctionExecutor<T, N, P>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N>,
{
    pub fn new(provider: Arc<P>) -> Self {
        Self {
            provider,
            _phantom_t: std::marker::PhantomData,
            _phantom_n: std::marker::PhantomData,
        }
    }

    pub async fn execute_function(
        &self,
        target_function: &TargetFunction,
        gas_config: Option<&GasConfig>,
    ) -> Result<alloy::primitives::TxHash>
    where
        N::TransactionRequest: Default + TransactionBuilder<N>,
        N::ReceiptResponse: ReceiptResponse,
    {
        let address = target_function
            .contract_address
            .parse::<Address>()
            .context("Invalid contract address")?;

        // Parse function signature and encode parameters
        let (func_name, param_types) = Self::parse_function_signature(&target_function.function)?;
        let encoded_params = Self::encode_parameters(&target_function.parameters, &param_types)?;

        // Create function definition
        let function = Function {
            name: func_name.clone(),
            inputs: Self::create_param_definitions(&param_types),
            outputs: vec![], // We don't care about outputs for execution
            state_mutability: StateMutability::NonPayable,
        };

        // Create minimal ABI
        let _abi = JsonAbi {
            constructor: None,
            functions: vec![(func_name.clone(), vec![function.clone()])].into_iter().collect(),
            events: Default::default(),
            errors: Default::default(),
            receive: None,
            fallback: None,
        };

        // Encode function call
        let encoded_call = function.abi_encode_input(&encoded_params)?;
        
        debug!(
            "Executing function {} on contract {} with {} parameters",
            func_name,
            address,
            target_function.parameters.len()
        );

        // Build transaction
        let mut tx = N::TransactionRequest::default();
        tx.set_to(address);
        tx.set_input(alloy::primitives::Bytes::from(encoded_call));

        // Apply gas configuration
        if let Some(gas_cfg) = gas_config {
            if let Some(gas_limit) = gas_cfg.gas_limit {
                tx.set_gas_limit(gas_limit);
            }
            
            // Handle gas pricing based on transaction type
            if let Some(max_gas_price) = gas_cfg.max_gas_price_gwei {
                let max_price = U256::from(max_gas_price) * U256::from(10).pow(U256::from(9));
                tx.set_max_fee_per_gas(max_price.to::<u128>());
                
                if let Some(priority_fee) = gas_cfg.priority_fee_gwei {
                    let priority = U256::from(priority_fee) * U256::from(10).pow(U256::from(9));
                    tx.set_max_priority_fee_per_gas(priority.to::<u128>());
                }
            }
        }

        // Send transaction
        let pending_tx = self.provider
            .send_transaction(tx)
            .await
            .context("Failed to send transaction")?;

        let tx_hash = *pending_tx.tx_hash();
        info!("Submitted transaction: 0x{:x}", tx_hash);

        // Wait for confirmation
        let receipt = pending_tx
            .get_receipt()
            .await
            .context("Failed to get transaction receipt")?;

        if receipt.status() {
            info!("Transaction confirmed successfully: 0x{:x}", tx_hash);
            Ok(tx_hash)
        } else {
            Err(anyhow!("Transaction failed: 0x{:x}", tx_hash))
        }
    }

    fn parse_function_signature(signature: &str) -> Result<(String, Vec<String>)> {
        let signature = signature.trim();
        
        // Find the opening parenthesis
        let open_paren = signature
            .find('(')
            .ok_or_else(|| anyhow!("Invalid function signature: missing '('"))?;
        
        let func_name = &signature[..open_paren];
        
        // Extract parameters section
        let close_paren = signature
            .rfind(')')
            .ok_or_else(|| anyhow!("Invalid function signature: missing ')'"))?;
        
        let params_str = &signature[open_paren + 1..close_paren];
        
        let param_types: Vec<String> = if params_str.is_empty() {
            vec![]
        } else {
            params_str
                .split(',')
                .map(|s| s.trim().to_string())
                .collect()
        };

        trace!("Parsed function: {} with params: {:?}", func_name, param_types);
        Ok((func_name.to_string(), param_types))
    }

    fn create_param_definitions(param_types: &[String]) -> Vec<Param> {
        param_types
            .iter()
            .enumerate()
            .map(|(i, ty)| Param {
                ty: ty.clone(),
                name: format!("param{}", i),
                components: vec![],
                internal_type: None,
            })
            .collect()
    }

    fn encode_parameters(
        parameters: &[Parameter],
        param_types: &[String],
    ) -> Result<Vec<DynSolValue>> {
        if parameters.len() != param_types.len() {
            return Err(anyhow!(
                "Parameter count mismatch: {} provided, {} expected",
                parameters.len(),
                param_types.len()
            ));
        }

        parameters
            .iter()
            .zip(param_types.iter())
            .map(|(param, ty)| Self::encode_single_parameter(&param.value, ty))
            .collect()
    }

    fn encode_single_parameter(
        value: &serde_json::Value,
        param_type: &str,
    ) -> Result<DynSolValue> {
        match param_type {
            "uint256" => {
                let val = if let Some(num) = value.as_u64() {
                    U256::from(num)
                } else if let Some(s) = value.as_str() {
                    U256::from_str_radix(s, 10)?
                } else {
                    return Err(anyhow!("Invalid uint256 value: {:?}", value));
                };
                Ok(DynSolValue::Uint(val, 256))
            }
            "address" => {
                let addr_str = value
                    .as_str()
                    .ok_or_else(|| anyhow!("Address must be a string"))?;
                let addr = addr_str.parse::<Address>()?;
                Ok(DynSolValue::Address(addr))
            }
            "bool" => {
                let val = value
                    .as_bool()
                    .ok_or_else(|| anyhow!("Bool value required"))?;
                Ok(DynSolValue::Bool(val))
            }
            "address[]" => {
                let arr = value
                    .as_array()
                    .ok_or_else(|| anyhow!("Array expected for address[]"))?;
                let addresses: Result<Vec<DynSolValue>> = arr
                    .iter()
                    .map(|v| {
                        let addr_str = v.as_str().ok_or_else(|| anyhow!("Address must be string"))?;
                        let addr = addr_str.parse::<Address>()?;
                        Ok(DynSolValue::Address(addr))
                    })
                    .collect();
                Ok(DynSolValue::Array(addresses?))
            }
            _ => Err(anyhow!("Unsupported parameter type: {}", param_type)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_function_signature() {
        // Use a dummy type for testing
        type DummyExecutor = FunctionExecutor<alloy::transports::http::Http<alloy::transports::http::Client>, alloy::network::Ethereum, alloy::providers::RootProvider<alloy::transports::http::Http<alloy::transports::http::Client>>>;
        
        let (name, params) = DummyExecutor::parse_function_signature(
            "transfer(address,uint256)"
        ).unwrap();
        assert_eq!(name, "transfer");
        assert_eq!(params, vec!["address", "uint256"]);

        let (name, params) = DummyExecutor::parse_function_signature(
            "execute()"
        ).unwrap();
        assert_eq!(name, "execute");
        assert_eq!(params.len(), 0);
    }

    #[test]
    fn test_encode_single_parameter() {
        // Use a dummy type for testing
        type DummyExecutor = FunctionExecutor<alloy::transports::http::Http<alloy::transports::http::Client>, alloy::network::Ethereum, alloy::providers::RootProvider<alloy::transports::http::Http<alloy::transports::http::Client>>>;
        
        // Test uint256
        let val = serde_json::json!(12345);
        let encoded = DummyExecutor::encode_single_parameter(&val, "uint256").unwrap();
        match encoded {
            DynSolValue::Uint(v, 256) => assert_eq!(v, U256::from(12345)),
            _ => panic!("Expected Uint256"),
        }

        // Test address
        let val = serde_json::json!("0x1234567890123456789012345678901234567890");
        let encoded = DummyExecutor::encode_single_parameter(&val, "address").unwrap();
        match encoded {
            DynSolValue::Address(addr) => {
                assert_eq!(
                    format!("{:?}", addr),
                    "0x1234567890123456789012345678901234567890"
                );
            }
            _ => panic!("Expected Address"),
        }
    }
}