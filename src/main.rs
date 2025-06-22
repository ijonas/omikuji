use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use tracing::{error, info, debug};
use tracing_subscriber::prelude::*; // <-- Import this trait to enable .with() for registry()

mod config;
mod network;
mod contracts;
mod datafeed;
mod gas;
mod database;
mod metrics;
mod wallet;
mod tui; // Add this to import the TUI dashboard module

use crate::tui::update;

/// Command line arguments
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Omikuji - A lightweight EVM blockchain datafeed provider",
    long_about = "Omikuji is a daemon that provides external off-chain data to EVM blockchains \
                  such as Ethereum and BASE. It manages datafeeds defined in YAML configuration \
                  files and updates smart contracts based on time and deviation thresholds."
)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Private key environment variable for signing transactions
    #[arg(short, long, default_value = "OMIKUJI_PRIVATE_KEY")]
    private_key_env: String,

    /// Enable TUI dashboard mode
    #[arg(long, help = "Enable TUI dashboard (terminal UI) mode")]
    tui: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments first
    let args = Args::parse();

    if args.tui {
        let dashboard = Arc::new(tokio::sync::RwLock::new(crate::tui::DashboardState::default()));
        // --- TUI Dashboard Log Channel Setup ---
        use crate::tui::{FeedStatus, NetworkStatus, start_tui_dashboard_with_state};
        use tokio::sync::mpsc;
        use tracing_subscriber::fmt::writer::BoxMakeWriter;
        let (log_tx, log_rx) = mpsc::channel(1000);
        tracing_subscriber::fmt()
            .with_ansi(false)
            .with_writer(BoxMakeWriter::new(move || crate::tui::ChannelWriter(log_tx.clone())))
            .with_timer(tracing_subscriber::fmt::time::ChronoLocal::new("%Y-%m-%d %H:%M:%S%.3f".to_string()))
            .with_target(true)
            .with_level(true)
            .with_max_level(tracing::Level::INFO)
            .compact()
            .init();
        // Load .env file if it exists
        match dotenv::dotenv() {
            Ok(path) => info!("Loaded .env file from: {:?}", path),
            Err(e) => info!("No .env file loaded: {}", e),
        }

        // Determine configuration path
        let config_arg = args.config.as_ref();
        let mut config_path = config_arg.cloned().unwrap_or_else(config::default_config_path);
        if !config_path.exists() {
            // If user did not specify --config and config.yaml exists in cwd, use it
            if config_arg.is_none() {
                let cwd_config = std::path::PathBuf::from("config.yaml");
                if cwd_config.exists() {
                    info!("No config file at {:?}, falling back to ./config.yaml", config_path);
                    config_path = cwd_config;
                }
            }
        }
        info!("Using configuration file: {:?}", config_path);
        if !config_path.exists() {
            error!("Configuration file not found: {:?}. To run Omikuji, use: cargo run -- --tui --config <your_config.yaml>", config_path);
            return Err(anyhow::anyhow!("Missing configuration file: {:?}", config_path));
        }

        // Load and validate configuration
        let config = match config::load_config(&config_path) {
            Ok(cfg) => {
                info!("Configuration loaded successfully");
                cfg
            }
            Err(e) => {
                error!("Failed to load configuration: {}", e);
                return Err(anyhow::anyhow!("Configuration error: {}", e));
            }
        };

        // Display loaded configuration
        info!("Loaded {} network(s) and {} datafeed(s)",
              config.networks.len(), config.datafeeds.len());

        for network in &config.networks {
            info!("Network: {} ({})", network.name, network.rpc_url);
        }

        for datafeed in &config.datafeeds {
            info!("Datafeed: {} on network {}", datafeed.name, datafeed.networks);
        }

        // Initialize network connections
        let mut network_manager = match network::NetworkManager::new(&config.networks).await {
            Ok(manager) => {
                info!("Network connections initialized successfully");
                manager
            }
            Err(e) => {
                error!("Failed to initialize network connections: {}", e);
                return Err(anyhow::anyhow!("Network initialization error: {}", e));
            }
        };

        // Try to load wallets for all networks from environment variable
        info!("Attempting to load wallet from environment variable: {}", args.private_key_env);
        
        // Check if the environment variable exists
        if std::env::var(&args.private_key_env).is_ok() {
            info!("Environment variable {} found", args.private_key_env);
        } else {
            error!("Environment variable {} not found", args.private_key_env);
        }
        
        for network in &config.networks {
            match network_manager.load_wallet_from_env(&network.name, &args.private_key_env).await {
                Ok(_) => {
                    info!("Loaded wallet for network {}", network.name);
                },
                Err(e) => {
                    error!("Failed to load wallet for network {}: {}", network.name, e);
                    error!("Transactions on {} network will not be possible", network.name);
                }
            }
        }

        // Now wrap in Arc for sharing across threads
        let network_manager = Arc::new(network_manager);

        // Initialize dashboard for TUI
        let dashboard = Arc::new(tokio::sync::RwLock::new(crate::tui::DashboardState::default()));

        // Initialize database connection (optional - continues if not available)
        info!("Checking for database configuration...");
        let database_pool = match std::env::var("DATABASE_URL") {
            Ok(_) => {
                debug!("DATABASE_URL environment variable found, attempting database connection");
                match database::establish_connection().await {
                    Ok(pool) => {
                        info!("Database connection established successfully");
                        
                        // Run migrations
                        if let Err(e) = database::connection::run_migrations(&pool).await {
                            error!("Failed to run database migrations: {}", e);
                            error!("Continuing without database support");
                            None
                        } else {
                            info!("Database initialized with feed logging and transaction tracking enabled");
                            Some(pool)
                        }
                    }
                    Err(e) => {
                        error!("Failed to establish database connection: {}", e);
                        error!("Continuing without database logging");
                        None
                    }
                }
            }
            Err(_) => {
                info!("DATABASE_URL not set - running without database logging");
                info!("To enable database logging, set DATABASE_URL environment variable");
                None
            }
        };

        // Initialize cleanup manager if database is available
        let cleanup_manager = if let Some(ref pool) = database_pool {
            let repository = Arc::new(database::FeedLogRepository::new(pool.clone()));
            let mut cleanup_manager = database::cleanup::CleanupManager::new(
                config.clone(),
                repository
            ).await?;
            
            // Start cleanup scheduler
            if let Err(e) = cleanup_manager.start().await {
                error!("Failed to start cleanup scheduler: {}", e);
            }
            
            Some(cleanup_manager)
        } else {
            None
        };

        // Initialize and start datafeed monitoring
        let mut feed_manager = if let Some(ref pool) = database_pool {
            datafeed::FeedManager::new(config.clone(), Arc::clone(&network_manager))
                .with_repository(pool.clone())
        } else {
            datafeed::FeedManager::new(config.clone(), Arc::clone(&network_manager))
        };
        // --- TUI: Spawn dashboard update task for live metrics ---
        let dash_for_metrics = dashboard.clone();
        let network_manager_for_metrics = network_manager.clone();
        let config_for_metrics = config.clone();
        tokio::spawn(async move {
            loop {
                // Update network status
                let mut networks = Vec::new();
                for net in &config_for_metrics.networks {
                    let chain_id = network_manager_for_metrics.get_chain_id(&net.name).await.ok();
                    let block_number = network_manager_for_metrics.get_block_number(&net.name).await.ok();
                    let rpc_ok = chain_id.is_some() && block_number.is_some();
                    networks.push(crate::tui::NetworkStatus {
                        name: net.name.clone(),
                        rpc_ok,
                        chain_id,
                        block_number,
                        wallet_status: "OK".to_string(),
                    });
                }
                {
                    let mut dash = dash_for_metrics.write().await;
                    dash.networks = networks;
                }
                // Update staleness and error counts
                update::update_avg_staleness(&dash_for_metrics).await;
                update::update_counts(&dash_for_metrics).await;
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        });
        // --- END TUI metrics task ---

        // --- TUI: Spawn DB metrics updater if DB is available ---
        if let Some(ref pool) = database_pool {
            use crate::database::TransactionLogRepository;
            use std::sync::Arc;
            use crate::tui::db_metrics::spawn_db_metrics_updater;
            let tx_log_repo = Arc::new(TransactionLogRepository::new(pool.clone()));
            spawn_db_metrics_updater(dashboard.clone(), tx_log_repo, 10);
        }
        // --- END DB metrics updater ---

        // Start all feed monitors with dashboard handle
        feed_manager.start(Some(dashboard.clone())).await;

        // Start Prometheus metrics server
        if let Err(e) = metrics::start_metrics_server(9090).await {
            error!("Failed to start metrics server: {}", e);
            error!("Continuing without metrics endpoint");
        } else {
            info!("Prometheus metrics available at http://0.0.0.0:9090/metrics");
        }

        // Start wallet balance monitor
        let wallet_monitor = wallet::WalletBalanceMonitor::new(Arc::clone(&network_manager));
        tokio::spawn(async move {
            wallet_monitor.start().await;
        });

        info!("Omikuji starting up...");

        // Get chain ID and block number for each network as a test
        for network in &config.networks {
            match network_manager.get_chain_id(&network.name).await {
                Ok(chain_id) => info!("Network {} chain ID: {}", network.name, chain_id),
                Err(e) => error!("Failed to get chain ID for network {}: {}", network.name, e),
            }

            match network_manager.get_block_number(&network.name).await {
                Ok(block_number) => info!("Network {} current block: {}", network.name, block_number),
                Err(e) => error!("Failed to get block number for network {}: {}", network.name, e),
            }
        }

        return start_tui_dashboard_with_state(dashboard, log_rx).await.map_err(|e| anyhow::anyhow!(e));
    } else {
        // --- Standard mode: use default tracing subscriber ---
        tracing_subscriber::fmt()
            .with_ansi(true)
            .with_timer(tracing_subscriber::fmt::time::ChronoLocal::new("%Y-%m-%d %H:%M:%S%.3f".to_string()))
            .with_target(true)
            .with_level(true)
            .with_max_level(tracing::Level::INFO)
            .compact()
            .init();
        // Load .env file if it exists
        match dotenv::dotenv() {
            Ok(path) => info!("Loaded .env file from: {:?}", path),
            Err(e) => info!("No .env file loaded: {}", e),
        }

        // Determine configuration path
        let config_arg = args.config.as_ref();
        let mut config_path = config_arg.cloned().unwrap_or_else(config::default_config_path);
        if !config_path.exists() {
            // If user did not specify --config and config.yaml exists in cwd, use it
            if config_arg.is_none() {
                let cwd_config = std::path::PathBuf::from("config.yaml");
                if cwd_config.exists() {
                    info!("No config file at {:?}, falling back to ./config.yaml", config_path);
                    config_path = cwd_config;
                }
            }
        }
        info!("Using configuration file: {:?}", config_path);
        if !config_path.exists() {
            error!("Configuration file not found: {:?}. To run Omikuji, use: cargo run -- --tui --config <your_config.yaml>", config_path);
            return Err(anyhow::anyhow!("Missing configuration file: {:?}", config_path));
        }

        // Load and validate configuration
        let config = match config::load_config(&config_path) {
            Ok(cfg) => {
                info!("Configuration loaded successfully");
                cfg
            }
            Err(e) => {
                error!("Failed to load configuration: {}", e);
                return Err(anyhow::anyhow!("Configuration error: {}", e));
            }
        };

        // Display loaded configuration
        info!("Loaded {} network(s) and {} datafeed(s)",
              config.networks.len(), config.datafeeds.len());

        for network in &config.networks {
            info!("Network: {} ({})", network.name, network.rpc_url);
        }

        for datafeed in &config.datafeeds {
            info!("Datafeed: {} on network {}", datafeed.name, datafeed.networks);
        }

        // Initialize network connections
        let mut network_manager = match network::NetworkManager::new(&config.networks).await {
            Ok(manager) => {
                info!("Network connections initialized successfully");
                manager
            }
            Err(e) => {
                error!("Failed to initialize network connections: {}", e);
                return Err(anyhow::anyhow!("Network initialization error: {}", e));
            }
        };

        // Try to load wallets for all networks from environment variable
        info!("Attempting to load wallet from environment variable: {}", args.private_key_env);
        
        // Check if the environment variable exists
        if std::env::var(&args.private_key_env).is_ok() {
            info!("Environment variable {} found", args.private_key_env);
        } else {
            error!("Environment variable {} not found", args.private_key_env);
        }
        
        for network in &config.networks {
            match network_manager.load_wallet_from_env(&network.name, &args.private_key_env).await {
                Ok(_) => {
                    info!("Loaded wallet for network {}", network.name);
                },
                Err(e) => {
                    error!("Failed to load wallet for network {}: {}", network.name, e);
                    error!("Transactions on {} network will not be possible", network.name);
                }
            }
        }

        // Now wrap in Arc for sharing across threads
        let network_manager = Arc::new(network_manager);

        // Initialize dashboard for TUI
        let dashboard = Arc::new(tokio::sync::RwLock::new(crate::tui::DashboardState::default()));

        // Initialize database connection (optional - continues if not available)
        info!("Checking for database configuration...");
        let database_pool = match std::env::var("DATABASE_URL") {
            Ok(_) => {
                debug!("DATABASE_URL environment variable found, attempting database connection");
                match database::establish_connection().await {
                    Ok(pool) => {
                        info!("Database connection established successfully");
                        
                        // Run migrations
                        if let Err(e) = database::connection::run_migrations(&pool).await {
                            error!("Failed to run database migrations: {}", e);
                            error!("Continuing without database support");
                            None
                        } else {
                            info!("Database initialized with feed logging and transaction tracking enabled");
                            Some(pool)
                        }
                    }
                    Err(e) => {
                        error!("Failed to establish database connection: {}", e);
                        error!("Continuing without database logging");
                        None
                    }
                }
            }
            Err(_) => {
                info!("DATABASE_URL not set - running without database logging");
                info!("To enable database logging, set DATABASE_URL environment variable");
                None
            }
        };

        // Initialize cleanup manager if database is available
        let cleanup_manager = if let Some(ref pool) = database_pool {
            let repository = Arc::new(database::FeedLogRepository::new(pool.clone()));
            let mut cleanup_manager = database::cleanup::CleanupManager::new(
                config.clone(),
                repository
            ).await?;
            
            // Start cleanup scheduler
            if let Err(e) = cleanup_manager.start().await {
                error!("Failed to start cleanup scheduler: {}", e);
            }
            
            Some(cleanup_manager)
        } else {
            None
        };

        // Initialize and start datafeed monitoring
        let mut feed_manager = if let Some(ref pool) = database_pool {
            datafeed::FeedManager::new(config.clone(), Arc::clone(&network_manager))
                .with_repository(pool.clone())
        } else {
            datafeed::FeedManager::new(config.clone(), Arc::clone(&network_manager))
        };
        // --- TUI: Spawn dashboard update task for live metrics ---
        let dash_for_metrics = dashboard.clone();
        let network_manager_for_metrics = network_manager.clone();
        let config_for_metrics = config.clone();
        tokio::spawn(async move {
            loop {
                // Update network status
                let mut networks = Vec::new();
                for net in &config_for_metrics.networks {
                    let chain_id = network_manager_for_metrics.get_chain_id(&net.name).await.ok();
                    let block_number = network_manager_for_metrics.get_block_number(&net.name).await.ok();
                    let rpc_ok = chain_id.is_some() && block_number.is_some();
                    networks.push(crate::tui::NetworkStatus {
                        name: net.name.clone(),
                        rpc_ok,
                        chain_id,
                        block_number,
                        wallet_status: "OK".to_string(),
                    });
                }
                {
                    let mut dash = dash_for_metrics.write().await;
                    dash.networks = networks;
                }
                // Update staleness and error counts
                update::update_avg_staleness(&dash_for_metrics).await;
                update::update_counts(&dash_for_metrics).await;
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        });
        // --- END TUI metrics task ---

        // --- TUI: Spawn DB metrics updater if DB is available ---
        if let Some(ref pool) = database_pool {
            use crate::database::TransactionLogRepository;
            use std::sync::Arc;
            use crate::tui::db_metrics::spawn_db_metrics_updater;
            let tx_log_repo = Arc::new(TransactionLogRepository::new(pool.clone()));
            spawn_db_metrics_updater(dashboard.clone(), tx_log_repo, 10);
        }
        // --- END DB metrics updater ---

        // Start all feed monitors with dashboard handle
        feed_manager.start(Some(dashboard.clone())).await;

        // Start Prometheus metrics server
        if let Err(e) = metrics::start_metrics_server(9090).await {
            error!("Failed to start metrics server: {}", e);
            error!("Continuing without metrics endpoint");
        } else {
            info!("Prometheus metrics available at http://0.0.0.0:9090/metrics");
        }

        // Start wallet balance monitor
        let wallet_monitor = wallet::WalletBalanceMonitor::new(Arc::clone(&network_manager));
        tokio::spawn(async move {
            wallet_monitor.start().await;
        });

        info!("Omikuji starting up...");

        // Get chain ID and block number for each network as a test
        for network in &config.networks {
            match network_manager.get_chain_id(&network.name).await {
                Ok(chain_id) => info!("Network {} chain ID: {}", network.name, chain_id),
                Err(e) => error!("Failed to get chain ID for network {}: {}", network.name, e),
            }

            match network_manager.get_block_number(&network.name).await {
                Ok(block_number) => info!("Network {} current block: {}", network.name, block_number),
                Err(e) => error!("Failed to get block number for network {}: {}", network.name, e),
            }
        }

        // Keep the application running
        tokio::signal::ctrl_c().await?;
        info!("Received shutdown signal, stopping...");

        // Stop cleanup manager if running
        if let Some(mut cleanup_manager) = cleanup_manager {
            if let Err(e) = cleanup_manager.stop().await {
                error!("Error stopping cleanup manager: {}", e);
            }
        }
    }
    Ok(())
}