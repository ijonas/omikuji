use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use tracing::{debug, error, info};

mod cli;
mod config;
mod contracts;
mod database;
mod datafeed;
mod gas;
mod gas_price;
mod metrics;
mod network;
mod ui;
mod wallet;

use cli::{Cli, Commands};
use wallet::KeyStorage;

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments first
    // This allows --version and --help to work without any side effects
    let cli = Cli::parse();

    // Handle key management commands before showing welcome screen
    match &cli.command {
        Some(Commands::Key { command }) => {
            // Initialize minimal logging for key commands
            tracing_subscriber::fmt::init();
            return cli::handle_key_command(command.clone()).await;
        }
        Some(Commands::Run) | None => {
            // Continue with normal daemon operation
        }
    }

    // Prepare version string for ASCII art
    let version = format!("Omikuji v{}", env!("CARGO_PKG_VERSION"));
    // The ASCII art is 100 chars wide, so center the version string
    let width = 100;
    let version_line = format!("{:^width$}", version, width = width);
    let welcome = ui::welcome_screen::WELCOME_SCREEN.replace("{version_line}", &version_line);
    println!("{}", welcome);

    // Initialize logging after argument parsing
    tracing_subscriber::fmt::init();

    // Load .env file if it exists
    match dotenv::dotenv() {
        Ok(path) => info!("Loaded .env file from: {:?}", path),
        Err(e) => info!("No .env file loaded: {}", e),
    }

    // Determine configuration path
    let config_path = cli.config.unwrap_or_else(config::default_config_path);
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

    // Initialize metrics configuration
    use crate::metrics::{init_metrics_config, ConfigMetrics};
    init_metrics_config(config.metrics.clone());

    // Record configuration metrics
    ConfigMetrics::record_startup_info(&config);

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

    // Load wallets based on key storage configuration
    use crate::wallet::key_storage::{EnvVarStorage, KeyringStorage};

    let key_storage: Box<dyn KeyStorage> = match config.key_storage.storage_type.as_str() {
        "keyring" => {
            info!("[MAIN DEBUG] Using OS keyring for key storage");
            info!(
                "[MAIN DEBUG] Keyring service name: '{}'",
                config.key_storage.keyring.service
            );
            info!("[MAIN DEBUG] Creating KeyringStorage instance");
            Box::new(KeyringStorage::new(Some(
                config.key_storage.keyring.service.clone(),
            )))
        }
        "env" => {
            info!("Using environment variables for key storage (consider migrating to keyring)");
            Box::new(EnvVarStorage::new())
        }
        _ => {
            error!(
                "Unknown key storage type: {}",
                config.key_storage.storage_type
            );
            return Err(anyhow::anyhow!("Invalid key storage configuration"));
        }
    };

    for network in &config.networks {
        info!(
            "[MAIN DEBUG] Attempting to load wallet for network: {}",
            network.name
        );
        match network_manager
            .load_wallet_from_key_storage(&network.name, key_storage.as_ref())
            .await
        {
            Ok(_) => {
                info!(
                    "[MAIN DEBUG] Successfully loaded wallet for network {}",
                    network.name
                );
            }
            Err(e) => {
                error!(
                    "[MAIN DEBUG] Failed to load wallet from key storage for network {}: {:?}",
                    network.name, e
                );

                // For backward compatibility, try environment variable if keyring fails
                if config.key_storage.storage_type == "keyring" {
                    info!(
                        "[MAIN DEBUG] Keyring lookup failed for network {}, trying environment variable",
                        network.name
                    );
                    info!("[MAIN DEBUG] Looking for env var: {}", cli.private_key_env);

                    match network_manager
                        .load_wallet_from_env(&network.name, &cli.private_key_env)
                        .await
                    {
                        Ok(_) => {
                            info!(
                                "[MAIN DEBUG] Successfully loaded wallet for network {} from environment variable",
                                network.name
                            );
                            continue;
                        }
                        Err(env_err) => {
                            error!("[MAIN DEBUG] Failed to load from env var: {:?}", env_err);
                        }
                    }
                }

                error!(
                    "[MAIN DEBUG] Final error - Failed to load wallet for network {}: {}",
                    network.name, e
                );
                error!(
                    "[MAIN DEBUG] Transactions on {} network will not be possible",
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
                        ConfigMetrics::set_database_status(false);
                        None
                    } else {
                        info!("Database initialized with feed logging and transaction tracking enabled");
                        ConfigMetrics::set_database_status(true);
                        Some(pool)
                    }
                }
                Err(e) => {
                    error!("Failed to establish database connection: {}", e);
                    error!("Continuing without database logging");
                    ConfigMetrics::set_database_status(false);
                    None
                }
            }
        }
        Err(_) => {
            info!("DATABASE_URL not set - running without database logging");
            info!("To enable database logging, set DATABASE_URL environment variable");
            ConfigMetrics::set_database_status(false);
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

    // Initialize gas price manager
    let gas_price_manager = if config.gas_price_feeds.enabled {
        info!("Initializing gas price feed manager");

        // Build token mappings from network configurations
        let mut token_mappings = std::collections::HashMap::new();
        for network in &config.networks {
            token_mappings.insert(network.name.clone(), network.gas_token.clone());
        }

        // Create transaction repository if database is available
        let tx_repo = database_pool.as_ref().map(|pool| {
            Arc::new(database::transaction_repository::TransactionLogRepository::new(pool.clone()))
        });

        let gas_price_manager = Arc::new(gas_price::GasPriceManager::new(
            config.gas_price_feeds.clone(),
            token_mappings,
            tx_repo,
        ));

        // Start the price update loop
        gas_price_manager.clone().start().await;

        Some(gas_price_manager)
    } else {
        info!("Gas price feeds are disabled");
        None
    };

    // Initialize and start datafeed monitoring
    let mut feed_manager = if let Some(pool) = database_pool {
        let mut manager = datafeed::FeedManager::new(config.clone(), Arc::clone(&network_manager))
            .with_repository(pool);

        // Add gas price manager if available
        if let Some(ref gas_price_manager) = gas_price_manager {
            manager = manager.with_gas_price_manager(Arc::clone(gas_price_manager));
        }

        manager
    } else {
        let mut manager = datafeed::FeedManager::new(config.clone(), Arc::clone(&network_manager));

        // Add gas price manager if available
        if let Some(ref gas_price_manager) = gas_price_manager {
            manager = manager.with_gas_price_manager(Arc::clone(gas_price_manager));
        }

        manager
    };

    feed_manager.start().await;

    // Start Prometheus metrics server
    if let Err(e) = metrics::start_metrics_server(9090).await {
        error!("Failed to start metrics server: {}", e);
        error!("Continuing without metrics endpoint");
    } else {
        info!("Prometheus metrics available at http://0.0.0.0:9090/metrics");

        // Update metrics server status
        ConfigMetrics::set_metrics_server_status(true, 9090);
    }

    // Start wallet balance monitor
    let mut wallet_monitor = wallet::WalletBalanceMonitor::new(Arc::clone(&network_manager));
    if let Some(ref gas_price_manager) = gas_price_manager {
        wallet_monitor = wallet_monitor.with_gas_price_manager(Arc::clone(gas_price_manager));
    }
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
