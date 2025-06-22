use lazy_static::lazy_static;
use prometheus::{register_gauge_vec, GaugeVec};
use tracing::info;

lazy_static! {
    /// Active datafeeds count
    static ref ACTIVE_DATAFEEDS: GaugeVec = register_gauge_vec!(
        "omikuji_active_datafeeds",
        "Number of active datafeeds",
        &["network", "status"]
    ).expect("Failed to create active_datafeeds metric");

    /// Datafeed configuration info (using gauge with value 1 for info metrics)
    static ref DATAFEED_CONFIG_INFO: GaugeVec = register_gauge_vec!(
        "omikuji_datafeed_config_info",
        "Datafeed configuration information",
        &[
            "feed_name", "network", "contract_type", "contract_address",
            "check_frequency", "deviation_threshold", "minimum_update_frequency"
        ]
    ).expect("Failed to create datafeed_config_info metric");

    /// Network configuration info
    static ref NETWORK_CONFIG_INFO: GaugeVec = register_gauge_vec!(
        "omikuji_network_config_info",
        "Network configuration information",
        &["network", "rpc_url", "transaction_type", "gas_multiplier"]
    ).expect("Failed to create network_config_info metric");

    /// Monitoring cycle duration
    static ref MONITORING_CYCLE_DURATION_SECONDS: GaugeVec = register_gauge_vec!(
        "omikuji_monitoring_cycle_duration_seconds",
        "Duration of monitoring cycles",
        &["cycle_type"]
    ).expect("Failed to create monitoring_cycle_duration metric");

    /// Version info
    static ref VERSION_INFO: GaugeVec = register_gauge_vec!(
        "omikuji_version_info",
        "Omikuji version information",
        &["version", "git_commit", "build_date", "rust_version"]
    ).expect("Failed to create version_info metric");

    /// Feature flags
    static ref FEATURE_FLAGS: GaugeVec = register_gauge_vec!(
        "omikuji_feature_flags",
        "Feature flag status (1 = enabled, 0 = disabled)",
        &["feature_name"]
    ).expect("Failed to create feature_flags metric");

    /// Configuration reload counter
    static ref CONFIG_RELOAD_COUNT: GaugeVec = register_gauge_vec!(
        "omikuji_config_reload_count",
        "Number of configuration reloads",
        &["reload_type", "status"]
    ).expect("Failed to create config_reload_count metric");

    /// Environment info
    static ref ENVIRONMENT_INFO: GaugeVec = register_gauge_vec!(
        "omikuji_environment_info",
        "Environment information",
        &["environment", "deployment_type", "region"]
    ).expect("Failed to create environment_info metric");

    /// Key storage configuration
    static ref KEY_STORAGE_CONFIG: GaugeVec = register_gauge_vec!(
        "omikuji_key_storage_config",
        "Key storage configuration",
        &["storage_type", "keyring_service"]
    ).expect("Failed to create key_storage_config metric");
}

/// Configuration metrics collector
pub struct ConfigMetrics;

impl ConfigMetrics {
    /// Update active datafeed count
    pub fn update_active_datafeeds(
        network: &str,
        active: usize,
        paused: usize,
        error: usize,
    ) {
        ACTIVE_DATAFEEDS
            .with_label_values(&[network, "active"])
            .set(active as f64);

        ACTIVE_DATAFEEDS
            .with_label_values(&[network, "paused"])
            .set(paused as f64);

        ACTIVE_DATAFEEDS
            .with_label_values(&[network, "error"])
            .set(error as f64);

        info!(
            "Active datafeeds on {}: {} active, {} paused, {} error",
            network, active, paused, error
        );
    }

    /// Set datafeed configuration info
    pub fn set_datafeed_config(
        feed_name: &str,
        network: &str,
        contract_type: &str,
        contract_address: &str,
        check_frequency: u64,
        deviation_threshold: f64,
        minimum_update_frequency: u64,
    ) {
        DATAFEED_CONFIG_INFO
            .with_label_values(&[
                feed_name,
                network,
                contract_type,
                contract_address,
                &check_frequency.to_string(),
                &format!("{:.2}", deviation_threshold),
                &minimum_update_frequency.to_string(),
            ])
            .set(1.0);
    }

    /// Set network configuration info
    pub fn set_network_config(
        network: &str,
        rpc_url: &str,
        transaction_type: &str,
        gas_multiplier: f64,
    ) {
        // Sanitize RPC URL to not expose credentials
        let sanitized_url = sanitize_url(rpc_url);
        
        NETWORK_CONFIG_INFO
            .with_label_values(&[
                network,
                &sanitized_url,
                transaction_type,
                &format!("{:.2}", gas_multiplier),
            ])
            .set(1.0);
    }

    /// Update monitoring cycle duration
    pub fn update_monitoring_cycle(
        cycle_type: &str,
        duration_seconds: f64,
    ) {
        MONITORING_CYCLE_DURATION_SECONDS
            .with_label_values(&[cycle_type])
            .set(duration_seconds);
    }

    /// Set version information
    pub fn set_version_info(
        version: &str,
        git_commit: &str,
        build_date: &str,
        rust_version: &str,
    ) {
        VERSION_INFO
            .with_label_values(&[version, git_commit, build_date, rust_version])
            .set(1.0);

        info!(
            "Omikuji version: {} (commit: {}, built: {}, rust: {})",
            version, git_commit, build_date, rust_version
        );
    }

    /// Update feature flag status
    pub fn update_feature_flag(feature_name: &str, enabled: bool) {
        FEATURE_FLAGS
            .with_label_values(&[feature_name])
            .set(if enabled { 1.0 } else { 0.0 });
    }

    /// Record configuration reload
    pub fn record_config_reload(reload_type: &str, success: bool) {
        let status = if success { "success" } else { "failure" };
        
        let gauge = CONFIG_RELOAD_COUNT
            .with_label_values(&[reload_type, status]);
        gauge.set(gauge.get() + 1.0);

        info!(
            "Configuration reload ({}): {}",
            reload_type, status
        );
    }

    /// Set environment information
    pub fn set_environment_info(
        environment: &str,
        deployment_type: &str,
        region: &str,
    ) {
        ENVIRONMENT_INFO
            .with_label_values(&[environment, deployment_type, region])
            .set(1.0);
    }

    /// Set key storage configuration
    pub fn set_key_storage_config(
        storage_type: &str,
        keyring_service: Option<&str>,
    ) {
        let service = keyring_service.unwrap_or("none");
        
        KEY_STORAGE_CONFIG
            .with_label_values(&[storage_type, service])
            .set(1.0);
    }
    
    /// Record startup information from config
    pub fn record_startup_info(config: &crate::config::models::OmikujiConfig) {
        // Set version info
        Self::set_version_info(
            env!("CARGO_PKG_VERSION"),
            option_env!("GIT_COMMIT").unwrap_or("unknown"),
            option_env!("BUILD_DATE").unwrap_or("unknown"),
            option_env!("RUSTC_VERSION").unwrap_or("unknown"),
        );
        
        // Set key storage config
        Self::set_key_storage_config(
            &config.key_storage.storage_type,
            Some(&config.key_storage.keyring.service),
        );
        
        // Set network configs
        for network in &config.networks {
            Self::set_network_config(
                &network.name,
                &network.rpc_url,
                &network.transaction_type,
                network.gas_config.gas_multiplier,
            );
        }
        
        // Set datafeed configs
        for datafeed in &config.datafeeds {
            Self::set_datafeed_config(
                &datafeed.name,
                &datafeed.networks,
                &datafeed.contract_type,
                &datafeed.contract_address,
                datafeed.check_frequency,
                datafeed.deviation_threshold_pct,
                datafeed.minimum_update_frequency,
            );
        }
        
        // Set environment info
        Self::set_environment_info(
            "production",
            "daemon",
            "global",
        );
    }
    
    /// Set database status
    pub fn set_database_status(enabled: bool) {
        Self::update_feature_flag("database", enabled);
    }
    
    /// Set metrics server status
    pub fn set_metrics_server_status(enabled: bool, port: u16) {
        Self::update_feature_flag("metrics_server", enabled);
        if enabled {
            info!("Metrics server started on port {}", port);
        }
    }
}

/// Sanitize URL to remove credentials
fn sanitize_url(url: &str) -> String {
    if let Ok(parsed) = url::Url::parse(url) {
        let mut sanitized = parsed.clone();
        sanitized.set_username("").ok();
        sanitized.set_password(None).ok();
        sanitized.to_string()
    } else {
        // If parsing fails, redact the entire URL
        "***REDACTED***".to_string()
    }
}