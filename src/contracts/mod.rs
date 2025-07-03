pub mod flux_aggregator;
pub mod flux_aggregator_v2;
pub mod interaction;

pub use flux_aggregator::FluxAggregatorContract;
pub use flux_aggregator_v2::FluxAggregatorV2;
pub use interaction::{ContractInteraction, ContractReader};

#[cfg(test)]
mod tests;
