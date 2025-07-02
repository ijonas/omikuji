use alloy::primitives::{TxHash, U256};
use tracing::{debug, error, info, warn};

/// Standard transaction logging utilities
pub struct TransactionLogger;

#[allow(dead_code)]
impl TransactionLogger {
    /// Log transaction submission
    pub fn log_submission(
        context_type: &str,
        context_name: &str,
        network: &str,
        value: Option<&str>,
    ) {
        if let Some(val) = value {
            info!(
                "Submitting {} update for '{}' on {}: {}",
                context_type, context_name, network, val
            );
        } else {
            info!(
                "Submitting {} for '{}' on {}",
                context_type, context_name, network
            );
        }
    }

    /// Log transaction confirmation
    pub fn log_confirmation(tx_hash: TxHash, gas_used: u128) {
        info!(
            "Successfully submitted to contract. Tx hash: 0x{:x}, Gas used: {}",
            tx_hash, gas_used
        );
    }

    /// Log transaction cost in USD
    pub fn log_usd_cost(
        total_cost_usd: f64,
        gas_used: u128,
        gas_price_wei: u128,
        token_price_usd: f64,
    ) {
        info!(
            "Transaction cost: ${:.6} USD (gas: {}, price: {} wei, token: ${:.2})",
            total_cost_usd, gas_used, gas_price_wei, token_price_usd
        );
    }

    /// Log transaction failure
    pub fn log_failure(context_type: &str, context_name: &str, error: &str) {
        error!(
            "Failed to submit {} for '{}': {}",
            context_type, context_name, error
        );
    }

    /// Log gas estimation details
    pub fn log_gas_estimation(estimated_gas: u64, gas_limit: u64, multiplier: f64) {
        info!(
            "Gas estimation: {} units (limit: {}, multiplier: {:.2}x)",
            estimated_gas, gas_limit, multiplier
        );
    }

    /// Log fee bumping attempt
    pub fn log_fee_bump(attempt: u32, old_price: U256, new_price: U256) {
        warn!(
            "Attempting fee bump #{}: {} -> {} wei",
            attempt, old_price, new_price
        );
    }

    /// Log transaction details for debugging
    pub fn log_transaction_details(
        to_address: &str,
        function_name: Option<&str>,
        gas_limit: Option<u64>,
        max_fee_per_gas: Option<U256>,
        max_priority_fee: Option<U256>,
    ) {
        let mut details = format!("Transaction details - To: {to_address}");

        if let Some(func) = function_name {
            details.push_str(&format!(", Function: {func}"));
        }

        if let Some(limit) = gas_limit {
            details.push_str(&format!(", Gas limit: {limit}"));
        }

        if let Some(max_fee) = max_fee_per_gas {
            details.push_str(&format!(
                ", Max fee: {} gwei",
                max_fee.to::<u128>() as f64 / 1e9
            ));
        }

        if let Some(priority) = max_priority_fee {
            details.push_str(&format!(
                ", Priority fee: {} gwei",
                priority.to::<u128>() as f64 / 1e9
            ));
        }

        info!("{}", details);
    }

    /// Log when conditions are met for execution
    pub fn log_condition_met(context_type: &str, context_name: &str, condition_desc: &str) {
        info!(
            "{} '{}' condition met: {}. Proceeding with execution.",
            context_type, context_name, condition_desc
        );
    }

    /// Log when conditions are not met
    pub fn log_condition_not_met(context_type: &str, context_name: &str, condition_desc: &str) {
        info!(
            "{} '{}' condition not met: {}. Skipping execution.",
            context_type, context_name, condition_desc
        );
    }

    /// Log when starting execution
    pub fn log_execution_start(context_type: &str, context_name: &str) {
        debug!(
            "=== Starting {} execution: {} ===",
            context_type, context_name
        );
    }

    /// Log when execution completes successfully
    pub fn log_execution_complete(context_type: &str, context_name: &str, tx_hash: TxHash) {
        info!(
            "Successfully executed {} '{}', tx hash: 0x{:x}",
            context_type, context_name, tx_hash
        );
    }
}
