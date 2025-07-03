use omikuji::scheduled_tasks::models::{
    CheckCondition, GasConfig, Parameter, ScheduledTask, TargetFunction,
};
use serde_json::json;
use std::str::FromStr;

#[cfg(test)]
mod integration_tests {
    use super::*;

    fn create_test_scheduled_task() -> ScheduledTask {
        ScheduledTask {
            name: "test_integration_task".to_string(),
            network: "test-network".to_string(),
            schedule: "*/10 * * * * *".to_string(), // Every 10 seconds
            check_condition: Some(CheckCondition::Function {
                contract_address: "0x5FbDB2315678afecb367f032d93F642f64180aa3".to_string(),
                function: "canDistributeRewards()".to_string(),
                expected_value: json!(true),
            }),
            target_function: TargetFunction {
                contract_address: "0x5FbDB2315678afecb367f032d93F642f64180aa3".to_string(),
                function: "distributeRewards(address[])".to_string(),
                parameters: vec![Parameter {
                    param_type: "address[]".to_string(),
                    value: json!([
                        "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
                        "0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC"
                    ]),
                }],
            },
            gas_config: Some(GasConfig {
                gas_limit: Some(300000),
                max_gas_price_gwei: Some(50),
                priority_fee_gwei: Some(2),
            }),
        }
    }

    #[test]
    fn test_scheduled_task_creation() {
        let task = create_test_scheduled_task();
        assert_eq!(task.name, "test_integration_task");
        assert_eq!(task.network, "test-network");
        assert_eq!(task.schedule, "*/10 * * * * *");
        assert!(task.check_condition.is_some());
        assert_eq!(task.target_function.parameters.len(), 1);
    }

    #[test]
    fn test_scheduled_task_gas_config() {
        let task = create_test_scheduled_task();
        let gas_config = task.gas_config.unwrap();
        assert_eq!(gas_config.gas_limit, Some(300000));
        assert_eq!(gas_config.max_gas_price_gwei, Some(50));
        assert_eq!(gas_config.priority_fee_gwei, Some(2));
    }

    #[test]
    fn test_scheduled_task_validation() {
        let task = create_test_scheduled_task();

        // Validate cron expression
        assert!(cron::Schedule::from_str(&task.schedule).is_ok());

        // Validate addresses
        use alloy::primitives::Address;
        assert!(task
            .target_function
            .contract_address
            .parse::<Address>()
            .is_ok());

        if let Some(CheckCondition::Function {
            contract_address, ..
        }) = &task.check_condition
        {
            assert!(contract_address.parse::<Address>().is_ok());
        }
    }

    #[test]
    fn test_parameter_serialization() {
        let param = Parameter {
            param_type: "uint256[]".to_string(),
            value: json!(["1000000", "2000000", "3000000"]),
        };

        // Test serialization
        let serialized = serde_json::to_string(&param).unwrap();
        let deserialized: Parameter = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.param_type, param.param_type);
        assert_eq!(deserialized.value, param.value);
    }

    #[test]
    fn test_complex_scheduled_task() {
        let task = ScheduledTask {
            name: "complex_task".to_string(),
            network: "mainnet".to_string(),
            schedule: "0 0 * * *".to_string(), // Daily at midnight
            check_condition: Some(CheckCondition::Property {
                contract_address: "0x9876543210987654321098765432109876543210".to_string(),
                property: "isActive".to_string(),
                expected_value: json!(true),
            }),
            target_function: TargetFunction {
                contract_address: "0x9876543210987654321098765432109876543210".to_string(),
                function: "performMaintenance(uint256,address[],bool)".to_string(),
                parameters: vec![
                    Parameter {
                        param_type: "uint256".to_string(),
                        value: json!("86400"), // 1 day in seconds
                    },
                    Parameter {
                        param_type: "address[]".to_string(),
                        value: json!([
                            "0x1111111111111111111111111111111111111111",
                            "0x2222222222222222222222222222222222222222",
                            "0x3333333333333333333333333333333333333333"
                        ]),
                    },
                    Parameter {
                        param_type: "bool".to_string(),
                        value: json!(true),
                    },
                ],
            },
            gas_config: None,
        };

        assert_eq!(task.target_function.parameters.len(), 3);
        assert_eq!(task.target_function.parameters[0].param_type, "uint256");
        assert_eq!(task.target_function.parameters[1].param_type, "address[]");
        assert_eq!(task.target_function.parameters[2].param_type, "bool");
    }

    #[test]
    fn test_check_condition_variants() {
        // Property condition
        let property_task = ScheduledTask {
            name: "property_check".to_string(),
            network: "testnet".to_string(),
            schedule: "*/30 * * * *".to_string(), // Every 30 minutes
            check_condition: Some(CheckCondition::Property {
                contract_address: "0xABCDEF1234567890123456789012345678901234".to_string(),
                property: "paused".to_string(),
                expected_value: json!(false),
            }),
            target_function: TargetFunction {
                contract_address: "0xABCDEF1234567890123456789012345678901234".to_string(),
                function: "processQueue()".to_string(),
                parameters: vec![],
            },
            gas_config: None,
        };

        match property_task.check_condition {
            Some(CheckCondition::Property {
                property,
                expected_value,
                ..
            }) => {
                assert_eq!(property, "paused");
                assert_eq!(expected_value, json!(false));
            }
            _ => panic!("Expected Property condition"),
        }

        // Function condition with uint256 return
        let function_task = ScheduledTask {
            name: "function_check".to_string(),
            network: "testnet".to_string(),
            schedule: "0 */6 * * *".to_string(), // Every 6 hours
            check_condition: Some(CheckCondition::Function {
                contract_address: "0xFEDCBA9876543210FEDCBA9876543210FEDCBA98".to_string(),
                function: "pendingRewards() (uint256)".to_string(),
                expected_value: json!("1000000000000000000"), // 1e18
            }),
            target_function: TargetFunction {
                contract_address: "0xFEDCBA9876543210FEDCBA9876543210FEDCBA98".to_string(),
                function: "claimRewards()".to_string(),
                parameters: vec![],
            },
            gas_config: None,
        };

        match function_task.check_condition {
            Some(CheckCondition::Function {
                function,
                expected_value,
                ..
            }) => {
                assert_eq!(function, "pendingRewards() (uint256)");
                assert_eq!(expected_value, json!("1000000000000000000"));
            }
            _ => panic!("Expected Function condition"),
        }
    }

    #[test]
    fn test_scheduled_task_json_serialization() {
        let task = create_test_scheduled_task();

        // Serialize to JSON
        let json_str = serde_json::to_string_pretty(&task).unwrap();

        // Deserialize back
        let deserialized: ScheduledTask = serde_json::from_str(&json_str).unwrap();

        assert_eq!(deserialized.name, task.name);
        assert_eq!(deserialized.network, task.network);
        assert_eq!(deserialized.schedule, task.schedule);
        assert_eq!(
            deserialized.target_function.function,
            task.target_function.function
        );
    }
}
