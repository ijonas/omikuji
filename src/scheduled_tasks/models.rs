use alloy::primitives::Address;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub name: String,
    pub network: String,
    pub schedule: String, // Cron expression
    pub check_condition: Option<CheckCondition>,
    pub target_function: TargetFunction,
    pub gas_config: Option<GasConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CheckCondition {
    Property {
        contract_address: String,
        property: String,
        expected_value: serde_json::Value,
    },
    Function {
        contract_address: String,
        function: String,
        expected_value: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetFunction {
    pub contract_address: String,
    pub function: String,
    pub parameters: Vec<Parameter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub value: serde_json::Value,
    #[serde(rename = "type")]
    pub param_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasConfig {
    pub max_gas_price_gwei: Option<u64>,
    pub gas_limit: Option<u64>,
    pub priority_fee_gwei: Option<u64>,
}

impl ScheduledTask {
    pub fn validate(&self) -> Result<(), String> {
        // Validate cron expression
        cron::Schedule::from_str(&self.schedule)
            .map_err(|e| format!("Invalid cron expression '{}': {}", self.schedule, e))?;

        // Validate addresses
        self.validate_address(&self.target_function.contract_address)?;
        
        if let Some(condition) = &self.check_condition {
            match condition {
                CheckCondition::Property { contract_address, .. } |
                CheckCondition::Function { contract_address, .. } => {
                    self.validate_address(contract_address)?;
                }
            }
        }

        Ok(())
    }

    fn validate_address(&self, address: &str) -> Result<(), String> {
        address.parse::<Address>()
            .map_err(|e| format!("Invalid address '{}': {}", address, e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduled_task_validation() {
        let task = ScheduledTask {
            name: "test_task".to_string(),
            network: "ethereum-mainnet".to_string(),
            schedule: "0 0 * * * *".to_string(), // Every hour at minute 0
            check_condition: Some(CheckCondition::Property {
                contract_address: "0x1234567890123456789012345678901234567890".to_string(),
                property: "isReady".to_string(),
                expected_value: serde_json::json!(true),
            }),
            target_function: TargetFunction {
                contract_address: "0x1234567890123456789012345678901234567890".to_string(),
                function: "execute()".to_string(),
                parameters: vec![],
            },
            gas_config: None,
        };

        match task.validate() {
            Ok(()) => {},
            Err(e) => panic!("Validation failed: {}", e),
        }
    }

    #[test]
    fn test_invalid_cron_expression() {
        let task = ScheduledTask {
            name: "test_task".to_string(),
            network: "ethereum-mainnet".to_string(),
            schedule: "invalid cron".to_string(),
            check_condition: None,
            target_function: TargetFunction {
                contract_address: "0x1234567890123456789012345678901234567890".to_string(),
                function: "execute()".to_string(),
                parameters: vec![],
            },
            gas_config: None,
        };

        assert!(task.validate().is_err());
    }
}