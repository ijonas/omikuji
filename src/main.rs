use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use tracing::{error, info};

mod config;

/// Command line arguments
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

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

    // TODO: Initialize network connections
    // TODO: Initialize datafeeds
    // TODO: Start the web interface
    // TODO: Start datafeed monitoring

    info!("Omikuji starting up...");
    
    // TODO: Main application loop

    Ok(())
}