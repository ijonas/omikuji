use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use tracing::{debug, error, info};

mod config;
mod contracts;
mod database;
mod datafeed;
mod gas;
mod metrics;
mod network;
mod ui;
mod wallet;

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
}

#[tokio::main]
async fn main() -> Result<()> {
    // Prepare version string for ASCII art
    let version = format!("Omikuji v{}", env!("CARGO_PKG_VERSION"));
    // The ASCII art is 100 chars wide, so center the version string
    let width = 100;
    let version_line = format!("{:^width$}", version, width = width);
    let welcome = ui::welcome_screen::WELCOME_SCREEN.replace("{version_line}", &version_line);
    println!("{}", welcome);

    // Parse command line arguments first
    // This allows --version and --help to work without any side effects
    let args = Args::parse();

    // Initialize logging after argument parsing
    tracing_subscriber::fmt::init();

    // Load .env file if it exists
    match dotenv::dotenv() {
        Ok(path) => info!("Loaded .env file from: {:?}", path),
        Err(e) => info!("No .env file loaded: {}", e),
    }

    // Determine configuration path
    let config_path = args.config.unwrap_or_else(config::default_config_path);
    info!("Using configuration file: {:?}", config_path);

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
    info!(
        "Loaded {} network(s) and {} datafeed(s)",
        config.networks.len(),
        config.datafeeds.len()
    );

    for network in &config.networks {
        info!("Network: {} ({})", network.name, network.rpc_url);
    }

    for datafeed in &config.datafeeds {
        info!(
            "Datafeed: {} on network {}",
            datafeed.name, datafeed.networks
        );
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
    info!(
        "Attempting to load wallet from environment variable: {}",
        args.private_key_env
    );

    // Check if the environment variable exists
    if std::env::var(&args.private_key_env).is_ok() {
        info!("Environment variable {} found", args.private_key_env);
    } else {
        error!("Environment variable {} not found", args.private_key_env);
    }

    for network in &config.networks {
        match network_manager
            .load_wallet_from_env(&network.name, &args.private_key_env)
            .await
        {
            Ok(_) => {
                info!("Loaded wallet for network {}", network.name);
            }
            Err(e) => {
                error!("Failed to load wallet for network {}: {}", network.name, e);
                error!(
                    "Transactions on {} network will not be possible",
                    network.name
                );
            }
        }
    }

    // Now wrap in Arc for sharing across threads
    let network_manager = Arc::new(network_manager);

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
        let mut cleanup_manager =
            database::cleanup::CleanupManager::new(config.clone(), repository).await?;

        // Start cleanup scheduler
        if let Err(e) = cleanup_manager.start().await {
            error!("Failed to start cleanup scheduler: {}", e);
        }

        Some(cleanup_manager)
    } else {
        None
    };

    // Initialize and start datafeed monitoring
    let mut feed_manager = if let Some(pool) = database_pool {
        datafeed::FeedManager::new(config.clone(), Arc::clone(&network_manager))
            .with_repository(pool)
    } else {
        datafeed::FeedManager::new(config.clone(), Arc::clone(&network_manager))
    };

    feed_manager.start().await;

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
            Err(e) => error!(
                "Failed to get block number for network {}: {}",
                network.name, e
            ),
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

    Ok(())
}
