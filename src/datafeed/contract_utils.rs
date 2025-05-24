use anyhow::{Context, Result};
use ethers::prelude::*;
use std::sync::Arc;
use crate::contracts::flux_aggregator::IFluxAggregator;
use crate::config::models::Datafeed;
use tracing::warn;

/// Parses and validates an Ethereum address
pub fn parse_address(address_str: &str) -> Result<Address> {
    address_str
        .parse::<Address>()
        .with_context(|| format!("Invalid contract address: {}", address_str))
}

/// Creates a FluxAggregator contract instance with a provider
pub fn create_contract_with_provider<P>(
    address: Address,
    provider: Arc<P>,
) -> IFluxAggregator<P>
where
    P: Middleware + 'static,
{
    IFluxAggregator::new(address, provider)
}

/// Creates a FluxAggregator contract instance with a signer
pub fn create_contract_with_signer<S>(
    address: Address,
    signer: Arc<S>,
) -> IFluxAggregator<S>
where
    S: Middleware + 'static,
{
    IFluxAggregator::new(address, signer)
}

/// Scales a floating point value by decimals for contract submission
pub fn scale_value_for_contract(value: f64, decimals: u8) -> i128 {
    let multiplier = 10f64.powi(decimals as i32);
    (value * multiplier).round() as i128
}

/// Validates a scaled value against min/max bounds
pub fn validate_value_bounds(
    scaled_value: i128,
    datafeed: &Datafeed,
) -> Result<()> {
    if let Some(min_value) = datafeed.min_value {
        if scaled_value < min_value as i128 {
            warn!(
                "Value {} is below minimum {} for datafeed {}",
                scaled_value, min_value, datafeed.name
            );
            return Err(anyhow::anyhow!("Value below minimum submission value"));
        }
    }
    
    if let Some(max_value) = datafeed.max_value {
        if scaled_value > max_value as i128 {
            warn!(
                "Value {} is above maximum {} for datafeed {}",
                scaled_value, max_value, datafeed.name
            );
            return Err(anyhow::anyhow!("Value above maximum submission value"));
        }
    }
    
    Ok(())
}

/// Gets current Unix timestamp in seconds
pub fn current_timestamp() -> Result<u64> {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("Failed to get current timestamp")
        .map(|d| d.as_secs())
}

/// Common error messages
pub mod errors {
    pub const NO_SIGNER_AVAILABLE: &str = "No signer available for network";
    pub const CONTRACT_SUBMISSION_FAILED: &str = "Contract submission failed";
}