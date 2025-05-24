pub mod fetcher;
pub mod json_extractor;
pub mod monitor;
pub mod manager;
pub mod contract_config;
#[cfg(test)]
mod tests;

pub use manager::FeedManager;