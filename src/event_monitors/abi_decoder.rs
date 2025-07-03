//! ABI decoder for parsing contract function signatures and encoding calls

use super::error::{EventMonitorError, Result};
use alloy::dyn_abi::{DynSolType, DynSolValue, JsonAbiExt};
use alloy::json_abi::Function;
use alloy::primitives::{Address, Bytes, U256};
use alloy::sol_types::Word;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::RwLock;
use tracing::{debug, trace};

use lazy_static::lazy_static;

lazy_static! {
    /// Cache for parsed function signatures
    static ref FUNCTION_CACHE: RwLock<HashMap<String, Function>> = RwLock::new(HashMap::new());
}

/// ABI decoder for contract function calls
pub struct AbiDecoder;

impl AbiDecoder {
    /// Parse a human-readable function signature
    /// Example: "transfer(address,uint256)"
    pub fn parse_function(signature: &str) -> Result<Function> {
        // Check cache first
        if let Ok(cache) = FUNCTION_CACHE.read() {
            if let Some(func) = cache.get(signature) {
                trace!("Using cached function for signature: {}", signature);
                return Ok(func.clone());
            }
        }

        // Parse the function signature
        let func = signature
            .parse::<Function>()
            .map_err(|e| EventMonitorError::DecodingError {
                monitor: String::new(),
                reason: format!("Invalid function signature '{signature}': {e}"),
            })?;

        // Cache the parsed function
        if let Ok(mut cache) = FUNCTION_CACHE.write() {
            cache.insert(signature.to_string(), func.clone());
        }

        debug!("Parsed function signature: {} -> {}", signature, func.name);
        Ok(func)
    }

    /// Encode function call data from JSON parameters
    pub fn encode_function_call(
        signature: &str,
        params: &[Value],
        monitor_name: &str,
    ) -> Result<Bytes> {
        let func = Self::parse_function(signature)?;

        // Validate parameter count
        if params.len() != func.inputs.len() {
            return Err(EventMonitorError::DecodingError {
                monitor: monitor_name.to_string(),
                reason: format!(
                    "Function '{}' expects {} parameters but got {}",
                    func.name,
                    func.inputs.len(),
                    params.len()
                ),
            });
        }

        // Convert JSON values to DynSolValue
        let mut encoded_params = Vec::new();
        for (i, (param, input)) in params.iter().zip(func.inputs.iter()).enumerate() {
            // Parse the type string to get DynSolType
            let sol_type =
                input
                    .ty
                    .parse::<DynSolType>()
                    .map_err(|e| EventMonitorError::DecodingError {
                        monitor: monitor_name.to_string(),
                        reason: format!("Failed to parse type '{}': {}", input.ty, e),
                    })?;

            let value =
                Self::json_to_dyn_sol_value(param, &sol_type, monitor_name).map_err(|e| {
                    EventMonitorError::DecodingError {
                        monitor: monitor_name.to_string(),
                        reason: format!("Failed to encode parameter {} ({}): {}", i, input.name, e),
                    }
                })?;
            encoded_params.push(value);
        }

        // Encode the function call
        let encoded = func.abi_encode_input(&encoded_params).map_err(|e| {
            EventMonitorError::DecodingError {
                monitor: monitor_name.to_string(),
                reason: format!("Failed to encode function call: {e}"),
            }
        })?;

        Ok(encoded.into())
    }

    /// Convert JSON value to DynSolValue based on the expected type
    fn json_to_dyn_sol_value(
        value: &Value,
        sol_type: &DynSolType,
        monitor_name: &str,
    ) -> Result<DynSolValue> {
        match (value, sol_type) {
            // Address type
            (Value::String(s), DynSolType::Address) => {
                let addr = s
                    .parse::<Address>()
                    .map_err(|e| EventMonitorError::DecodingError {
                        monitor: monitor_name.to_string(),
                        reason: format!("Invalid address '{s}': {e}"),
                    })?;
                Ok(DynSolValue::Address(addr))
            }

            // Unsigned integers
            (Value::String(s), DynSolType::Uint(bits)) => {
                let val =
                    U256::from_str_radix(s, 10).map_err(|e| EventMonitorError::DecodingError {
                        monitor: monitor_name.to_string(),
                        reason: format!("Invalid uint{bits} value '{s}': {e}"),
                    })?;
                Ok(DynSolValue::Uint(val, *bits))
            }
            (Value::Number(n), DynSolType::Uint(bits)) => {
                let val = n.as_u64().ok_or_else(|| EventMonitorError::DecodingError {
                    monitor: monitor_name.to_string(),
                    reason: format!("Number too large for uint{bits}"),
                })?;
                Ok(DynSolValue::Uint(U256::from(val), *bits))
            }

            // Boolean
            (Value::Bool(b), DynSolType::Bool) => Ok(DynSolValue::Bool(*b)),

            // String
            (Value::String(s), DynSolType::String) => Ok(DynSolValue::String(s.clone())),

            // Bytes
            (Value::String(s), DynSolType::Bytes) => {
                let bytes = hex::decode(s.trim_start_matches("0x")).map_err(|e| {
                    EventMonitorError::DecodingError {
                        monitor: monitor_name.to_string(),
                        reason: format!("Invalid hex bytes '{s}': {e}"),
                    }
                })?;
                Ok(DynSolValue::Bytes(bytes))
            }

            // Fixed bytes
            (Value::String(s), DynSolType::FixedBytes(size)) => {
                let bytes = hex::decode(s.trim_start_matches("0x")).map_err(|e| {
                    EventMonitorError::DecodingError {
                        monitor: monitor_name.to_string(),
                        reason: format!("Invalid hex bytes '{s}': {e}"),
                    }
                })?;
                if bytes.len() != *size {
                    return Err(EventMonitorError::DecodingError {
                        monitor: monitor_name.to_string(),
                        reason: format!("Expected {} bytes but got {}", size, bytes.len()),
                    });
                }

                // Create a Word (32 bytes) and copy our data into it
                let mut word = Word::ZERO;
                word[..*size].copy_from_slice(&bytes);
                Ok(DynSolValue::FixedBytes(word, *size))
            }

            // Arrays
            (Value::Array(arr), DynSolType::Array(inner_type)) => {
                let mut values = Vec::new();
                for item in arr {
                    values.push(Self::json_to_dyn_sol_value(item, inner_type, monitor_name)?);
                }
                Ok(DynSolValue::Array(values))
            }

            // Fixed arrays
            (Value::Array(arr), DynSolType::FixedArray(inner_type, size)) => {
                if arr.len() != *size {
                    return Err(EventMonitorError::DecodingError {
                        monitor: monitor_name.to_string(),
                        reason: format!("Expected array of size {} but got {}", size, arr.len()),
                    });
                }
                let mut values = Vec::new();
                for item in arr {
                    values.push(Self::json_to_dyn_sol_value(item, inner_type, monitor_name)?);
                }
                Ok(DynSolValue::FixedArray(values))
            }

            // Tuples
            (Value::Array(arr), DynSolType::Tuple(types)) => {
                if arr.len() != types.len() {
                    return Err(EventMonitorError::DecodingError {
                        monitor: monitor_name.to_string(),
                        reason: format!(
                            "Expected tuple with {} elements but got {}",
                            types.len(),
                            arr.len()
                        ),
                    });
                }
                let mut values = Vec::new();
                for (item, ty) in arr.iter().zip(types.iter()) {
                    values.push(Self::json_to_dyn_sol_value(item, ty, monitor_name)?);
                }
                Ok(DynSolValue::Tuple(values))
            }

            // Type mismatch
            _ => Err(EventMonitorError::DecodingError {
                monitor: monitor_name.to_string(),
                reason: format!("Type mismatch: cannot convert {value:?} to {sol_type:?}"),
            }),
        }
    }

    /// Clear the function cache (mainly for testing)
    #[cfg(test)]
    pub fn clear_cache() {
        if let Ok(mut cache) = FUNCTION_CACHE.write() {
            cache.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_function_signature() {
        let func = AbiDecoder::parse_function("transfer(address,uint256)").unwrap();
        assert_eq!(func.name, "transfer");
        assert_eq!(func.inputs.len(), 2);
        assert_eq!(func.inputs[0].ty, "address");
        assert_eq!(func.inputs[1].ty, "uint256");
    }

    #[test]
    fn test_parse_complex_function() {
        let func = AbiDecoder::parse_function("swap(address,uint256,address[],bytes)").unwrap();
        assert_eq!(func.name, "swap");
        assert_eq!(func.inputs.len(), 4);
        assert_eq!(func.inputs[2].ty, "address[]");
        assert_eq!(func.inputs[3].ty, "bytes");
    }

    #[test]
    fn test_encode_transfer_call() {
        let params = vec![
            Value::String("0x1234567890123456789012345678901234567890".to_string()),
            Value::String("1000000".to_string()),
        ];

        let encoded =
            AbiDecoder::encode_function_call("transfer(address,uint256)", &params, "test_monitor")
                .unwrap();

        // Function selector for transfer(address,uint256) is 0xa9059cbb
        assert!(encoded.starts_with(&[0xa9, 0x05, 0x9c, 0xbb]));
    }

    #[test]
    fn test_parameter_count_validation() {
        let params = vec![Value::String(
            "0x1234567890123456789012345678901234567890".to_string(),
        )];

        let result =
            AbiDecoder::encode_function_call("transfer(address,uint256)", &params, "test_monitor");

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("expects 2 parameters but got 1"));
    }

    #[test]
    fn test_invalid_address() {
        let params = vec![
            Value::String("invalid_address".to_string()),
            Value::String("1000000".to_string()),
        ];

        let result =
            AbiDecoder::encode_function_call("transfer(address,uint256)", &params, "test_monitor");

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid address"));
    }

    #[test]
    fn test_array_encoding() {
        let params = vec![
            Value::Array(vec![
                Value::String("0x1234567890123456789012345678901234567890".to_string()),
                Value::String("0x2345678901234567890123456789012345678901".to_string()),
            ]),
            Value::Array(vec![
                Value::String("100".to_string()),
                Value::String("200".to_string()),
            ]),
        ];

        let encoded = AbiDecoder::encode_function_call(
            "batchTransfer(address[],uint256[])",
            &params,
            "test_monitor",
        )
        .unwrap();

        // Should have function selector
        assert!(encoded.len() > 4);
    }

    #[test]
    fn test_function_cache() {
        AbiDecoder::clear_cache();

        // First call should parse
        let _func1 = AbiDecoder::parse_function("test(uint256)").unwrap();

        // Second call should use cache
        let _func2 = AbiDecoder::parse_function("test(uint256)").unwrap();

        // Clear cache for other tests
        AbiDecoder::clear_cache();
    }
}
