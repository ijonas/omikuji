pub mod fetcher;
pub mod json_extractor;
pub mod monitor;
pub mod manager;
#[cfg(test)]
mod tests;

pub use manager::FeedManager;