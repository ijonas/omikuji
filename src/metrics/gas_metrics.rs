use prometheus::{
    register_counter_vec, register_histogram_vec, register_gauge_vec,
    CounterVec, HistogramVec, GaugeVec, TextEncoder, Encoder,
};
use lazy_static::lazy_static;
use ethers::types::{TransactionReceipt, U256};
use tracing::{info, warn};

lazy_static! {
    /// Total gas used counter per feed and network
    static ref GAS_USED_TOTAL: CounterVec = register_counter_vec!(
        "omikuji_gas_used_total",
        "Total gas consumed by transactions",
        &["feed_name", "network", "status"]
    ).expect("Failed to create gas_used_total metric");

    /// Gas price histogram in gwei per network
    static ref GAS_PRICE_GWEI: HistogramVec = register_histogram_vec!(
        "omikuji_gas_price_gwei",
        "Gas price in gwei for transactions",
        &["network", "tx_type"],
        vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0]
    ).expect("Failed to create gas_price_gwei metric");

    /// Gas efficiency gauge (percentage of gas limit used)
    static ref GAS_EFFICIENCY_PERCENT: GaugeVec = register_gauge_vec!(
        "omikuji_gas_efficiency_percent",
        "Percentage of gas limit actually used",
        &["feed_name", "network"]
    ).expect("Failed to create gas_efficiency_percent metric");

    /// Transaction cost in native token (wei)
    static ref TRANSACTION_COST_WEI: HistogramVec = register_histogram_vec!(
        "omikuji_transaction_cost_wei",
        "Transaction cost in wei",
        &["feed_name", "network"],
        // Buckets in wei (0.0001 to 1 native token)
        vec![1e14, 5e14, 1e15, 5e15, 1e16, 5e16, 1e17, 5e17, 1e18]
    ).expect("Failed to create transaction_cost_wei metric");

    /// Number of transactions counter
    static ref TRANSACTION_COUNT: CounterVec = register_counter_vec!(
        "omikuji_transaction_count",
        "Number of transactions submitted",
        &["feed_name", "network", "status", "tx_type"]
    ).expect("Failed to create transaction_count metric");

    /// Gas limit vs gas used comparison
    static ref GAS_LIMIT_GAUGE: GaugeVec = register_gauge_vec!(
        "omikuji_gas_limit",
        "Gas limit set for transactions",
        &["feed_name", "network"]
    ).expect("Failed to create gas_limit gauge");
}

/// Gas metrics collector
pub struct GasMetrics;

impl GasMetrics {
    /// Record gas metrics from a transaction receipt
    pub fn record_transaction(
        feed_name: &str,
        network: &str,
        receipt: &TransactionReceipt,
        gas_limit: U256,
        tx_type: &str,
    ) {
        let status = if receipt.status == Some(1.into()) {
            "success"
        } else {
            "failed"
        };

        // Get gas used and effective gas price
        let gas_used = receipt.gas_used.unwrap_or_default();
        let effective_gas_price = receipt.effective_gas_price.unwrap_or_default();
        
        // Calculate metrics
        let gas_used_f64 = gas_used.as_u64() as f64;
        let gas_limit_f64 = gas_limit.as_u64() as f64;
        let gas_price_gwei = effective_gas_price.as_u128() as f64 / 1e9;
        let total_cost_wei = gas_used.saturating_mul(effective_gas_price);
        let efficiency_percent = if gas_limit > U256::zero() {
            (gas_used_f64 / gas_limit_f64) * 100.0
        } else {
            0.0
        };

        // Record metrics
        GAS_USED_TOTAL
            .with_label_values(&[feed_name, network, status])
            .inc_by(gas_used_f64);

        GAS_PRICE_GWEI
            .with_label_values(&[network, tx_type])
            .observe(gas_price_gwei);

        GAS_EFFICIENCY_PERCENT
            .with_label_values(&[feed_name, network])
            .set(efficiency_percent);

        TRANSACTION_COST_WEI
            .with_label_values(&[feed_name, network])
            .observe(total_cost_wei.as_u128() as f64);

        TRANSACTION_COUNT
            .with_label_values(&[feed_name, network, status, tx_type])
            .inc();

        GAS_LIMIT_GAUGE
            .with_label_values(&[feed_name, network])
            .set(gas_limit_f64);

        // Log the transaction
        info!(
            "Transaction gas metrics - Feed: {}, Network: {}, Status: {}, \
            Gas Used: {}, Gas Limit: {}, Efficiency: {:.1}%, \
            Gas Price: {:.2} gwei, Total Cost: {} wei ({:.6} native tokens), \
            Tx Hash: {:?}",
            feed_name,
            network,
            status,
            gas_used,
            gas_limit,
            efficiency_percent,
            gas_price_gwei,
            total_cost_wei,
            total_cost_wei.as_u128() as f64 / 1e18,
            receipt.transaction_hash
        );

        // Warn if efficiency is poor
        if efficiency_percent < 50.0 && gas_limit > U256::zero() {
            warn!(
                "Low gas efficiency for {} on {}: {:.1}% of limit used. \
                Consider reducing gas limit.",
                feed_name, network, efficiency_percent
            );
        } else if efficiency_percent > 90.0 {
            warn!(
                "High gas usage for {} on {}: {:.1}% of limit used. \
                Consider increasing gas limit for safety.",
                feed_name, network, efficiency_percent
            );
        }
    }

    /// Record a failed transaction (one that didn't get a receipt)
    pub fn record_failed_transaction(
        feed_name: &str,
        network: &str,
        gas_limit: U256,
        estimated_gas_price: Option<U256>,
        tx_type: &str,
        error: &str,
    ) {
        TRANSACTION_COUNT
            .with_label_values(&[feed_name, network, "error", tx_type])
            .inc();

        if let Some(gas_price) = estimated_gas_price {
            let gas_price_gwei = gas_price.as_u128() as f64 / 1e9;
            GAS_PRICE_GWEI
                .with_label_values(&[network, tx_type])
                .observe(gas_price_gwei);
        }

        warn!(
            "Transaction failed before receipt - Feed: {}, Network: {}, \
            Gas Limit: {}, Error: {}",
            feed_name, network, gas_limit, error
        );
    }

    /// Get Prometheus metrics as text
    pub fn gather_metrics() -> String {
        let encoder = TextEncoder::new();
        let metric_families = prometheus::gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    }
}

/// Transaction details for logging
#[derive(Debug, Clone)]
pub struct TransactionDetails {
    pub feed_name: String,
    pub network: String,
    pub tx_hash: String,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub gas_price_gwei: f64,
    pub total_cost_wei: u128,
    pub efficiency_percent: f64,
    pub status: String,
    pub tx_type: String,
    pub block_number: u64,
    pub error_message: Option<String>,
}