use prometheus::{
    register_gauge_vec, GaugeVec,
};
use lazy_static::lazy_static;
use tracing::{debug, warn};

lazy_static! {
    /// Wallet balance in wei for each network
    static ref WALLET_BALANCE_WEI: GaugeVec = register_gauge_vec!(
        "omikuji_wallet_balance_wei",
        "Wallet balance in wei",
        &["network", "address"]
    ).expect("Failed to create wallet_balance_wei metric");

    /// Latest feed value from external source
    static ref FEED_VALUE: GaugeVec = register_gauge_vec!(
        "omikuji_feed_value",
        "Latest value from external feed source",
        &["feed", "network"]
    ).expect("Failed to create feed_value metric");

    /// Timestamp of last feed update from external source
    static ref FEED_LAST_UPDATE_TIMESTAMP: GaugeVec = register_gauge_vec!(
        "omikuji_feed_last_update_timestamp",
        "Unix timestamp of last feed update",
        &["feed", "network"]
    ).expect("Failed to create feed_last_update_timestamp metric");

    /// Timestamp of last contract update
    static ref CONTRACT_LAST_UPDATE_TIMESTAMP: GaugeVec = register_gauge_vec!(
        "omikuji_contract_last_update_timestamp",
        "Unix timestamp of last contract update",
        &["feed", "network"]
    ).expect("Failed to create contract_last_update_timestamp metric");

    /// Latest on-chain contract value
    static ref CONTRACT_VALUE: GaugeVec = register_gauge_vec!(
        "omikuji_contract_value",
        "Latest value from on-chain contract",
        &["feed", "network"]
    ).expect("Failed to create contract_value metric");

    /// Latest round number
    static ref CONTRACT_ROUND: GaugeVec = register_gauge_vec!(
        "omikuji_contract_round",
        "Latest round number from contract",
        &["feed", "network"]
    ).expect("Failed to create contract_round metric");

    /// Absolute deviation percentage between contract and feed values
    static ref FEED_DEVIATION_PERCENT: GaugeVec = register_gauge_vec!(
        "omikuji_feed_deviation_percent",
        "Absolute deviation percentage between contract and feed values",
        &["feed", "network"]
    ).expect("Failed to create feed_deviation_percent metric");

    /// Data staleness indicator (seconds since last update)
    static ref DATA_STALENESS_SECONDS: GaugeVec = register_gauge_vec!(
        "omikuji_data_staleness_seconds",
        "Seconds since last successful data update",
        &["feed", "network", "data_type"]
    ).expect("Failed to create data_staleness_seconds metric");
}

/// Feed metrics collector
pub struct FeedMetrics;

impl FeedMetrics {
    /// Update wallet balance metric
    pub fn set_wallet_balance(network: &str, address: &str, balance_wei: u128) {
        WALLET_BALANCE_WEI
            .with_label_values(&[network, address])
            .set(balance_wei as f64);
        
        debug!(
            "Updated wallet balance for {} on {}: {} wei",
            address, network, balance_wei
        );
    }

    /// Update feed value from external source
    pub fn set_feed_value(feed_name: &str, network: &str, value: f64, timestamp: u64) {
        FEED_VALUE
            .with_label_values(&[feed_name, network])
            .set(value);
        
        FEED_LAST_UPDATE_TIMESTAMP
            .with_label_values(&[feed_name, network])
            .set(timestamp as f64);
        
        // Reset staleness counter
        DATA_STALENESS_SECONDS
            .with_label_values(&[feed_name, network, "feed"])
            .set(0.0);
        
        debug!(
            "Updated feed value for {} on {}: {} at timestamp {}",
            feed_name, network, value, timestamp
        );
    }

    /// Update contract value and round
    pub fn set_contract_value(
        feed_name: &str,
        network: &str,
        value: f64,
        round: u64,
        timestamp: u64,
    ) {
        CONTRACT_VALUE
            .with_label_values(&[feed_name, network])
            .set(value);
        
        CONTRACT_ROUND
            .with_label_values(&[feed_name, network])
            .set(round as f64);
        
        CONTRACT_LAST_UPDATE_TIMESTAMP
            .with_label_values(&[feed_name, network])
            .set(timestamp as f64);
        
        // Reset staleness counter
        DATA_STALENESS_SECONDS
            .with_label_values(&[feed_name, network, "contract"])
            .set(0.0);
        
        debug!(
            "Updated contract value for {} on {}: {} (round {}) at timestamp {}",
            feed_name, network, value, round, timestamp
        );
    }

    /// Calculate and update deviation percentage
    pub fn update_deviation(feed_name: &str, network: &str, feed_value: f64, contract_value: f64) {
        if contract_value == 0.0 {
            warn!(
                "Cannot calculate deviation for {} on {}: contract value is zero",
                feed_name, network
            );
            return;
        }
        
        let deviation_percent = ((feed_value - contract_value).abs() / contract_value) * 100.0;
        
        FEED_DEVIATION_PERCENT
            .with_label_values(&[feed_name, network])
            .set(deviation_percent);
        
        debug!(
            "Updated deviation for {} on {}: {:.2}% (feed: {}, contract: {})",
            feed_name, network, deviation_percent, feed_value, contract_value
        );
    }

    /// Update staleness metric for a data source
    pub fn update_staleness(feed_name: &str, network: &str, data_type: &str, seconds: f64) {
        DATA_STALENESS_SECONDS
            .with_label_values(&[feed_name, network, data_type])
            .set(seconds);
    }

    /// Record a successful contract update
    pub fn record_contract_update(feed_name: &str, network: &str) {
        let timestamp = chrono::Utc::now().timestamp() as f64;
        CONTRACT_LAST_UPDATE_TIMESTAMP
            .with_label_values(&[feed_name, network])
            .set(timestamp);
        
        debug!(
            "Recorded contract update for {} on {} at timestamp {}",
            feed_name, network, timestamp
        );
    }
}