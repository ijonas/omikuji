use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use tracing::{error, info};

mod config;
mod network;
mod contracts;

/// Command line arguments
#[derive(Parser, Debug)]
#[command(author, version, about)]
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
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Load .env file if it exists
    match dotenv::dotenv() {
        Ok(path) => info!("Loaded .env file from: {:?}", path),
        Err(e) => info!("No .env file loaded: {}", e),
    }

    // Parse command line arguments
    let args = Args::parse();

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

    // TODO: Initialize datafeeds
    // TODO: Start the web interface
    // TODO: Start datafeed monitoring

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

    Ok(())
}