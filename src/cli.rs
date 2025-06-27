use anyhow::Result;
use clap::{Parser, Subcommand};
use secrecy::SecretString;
use std::path::PathBuf;

use crate::wallet::key_storage::{KeyStorage, KeyringStorage};

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Omikuji - A lightweight EVM blockchain datafeed provider",
    long_about = "Omikuji is a daemon that provides external off-chain data to EVM blockchains \
                  such as Ethereum and BASE. It manages datafeeds defined in YAML configuration \
                  files and updates smart contracts based on time and deviation thresholds."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Path to configuration file
    #[arg(short, long, value_name = "FILE", global = true)]
    pub config: Option<PathBuf>,

    /// Private key environment variable for signing transactions
    #[arg(short, long, default_value = "OMIKUJI_PRIVATE_KEY", global = true)]
    pub private_key_env: String,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Manage private keys
    Key {
        #[command(subcommand)]
        command: KeyCommands,
    },
    /// Run the omikuji daemon (default behavior)
    Run,
}

#[derive(Subcommand, Debug, Clone)]
pub enum KeyCommands {
    /// Import a private key
    Import {
        /// Network name (e.g., "ethereum-mainnet", "base-sepolia")
        #[arg(short, long)]
        network: String,

        /// Private key (if not provided, will prompt for input)
        #[arg(short, long)]
        key: Option<String>,

        /// Path to file containing the private key
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// Keyring service name (defaults to "omikuji")
        #[arg(short, long)]
        service: Option<String>,
    },
    /// List stored keys
    List {
        /// Keyring service name (defaults to "omikuji")
        #[arg(short, long)]
        service: Option<String>,
    },
    /// Remove a key
    Remove {
        /// Network name
        #[arg(short, long)]
        network: String,

        /// Keyring service name (defaults to "omikuji")
        #[arg(short, long)]
        service: Option<String>,
    },
    /// Export a key (with confirmation prompt)
    Export {
        /// Network name
        #[arg(short, long)]
        network: String,

        /// Keyring service name (defaults to "omikuji")
        #[arg(short, long)]
        service: Option<String>,
    },
    /// Migrate keys from environment variables to keyring
    Migrate {
        /// Keyring service name (defaults to "omikuji")
        #[arg(short, long)]
        service: Option<String>,
    },
}

pub async fn handle_key_command(command: KeyCommands) -> Result<()> {
    match command {
        KeyCommands::Import {
            network,
            key,
            file,
            service,
        } => import_key(network, key, file, service).await,
        KeyCommands::List { service } => list_keys(service).await,
        KeyCommands::Remove { network, service } => remove_key(network, service).await,
        KeyCommands::Export { network, service } => export_key(network, service).await,
        KeyCommands::Migrate { service } => migrate_keys(service).await,
    }
}

async fn import_key(
    network: String,
    key: Option<String>,
    file: Option<PathBuf>,
    service: Option<String>,
) -> Result<()> {
    let storage = KeyringStorage::new(service);

    let private_key = match (key, file) {
        (Some(k), _) => SecretString::from(k),
        (None, Some(path)) => {
            let content = std::fs::read_to_string(path)?;
            SecretString::from(content.trim().to_string())
        }
        (None, None) => {
            // Prompt for key input
            println!("Enter private key for network '{}': ", network);
            let key = rpassword::prompt_password("")?;
            SecretString::from(key)
        }
    };

    storage.store_key(&network, private_key).await?;
    println!("Successfully imported key for network '{}'", network);

    // Verify the key was stored (important for detecting non-persistent backends)
    match storage.get_key(&network).await {
        Ok(_) => {}
        Err(e) => {
            println!(
                "WARNING: Key was stored but verification failed. Error: {}",
                e
            );
        }
    }

    Ok(())
}

async fn list_keys(service: Option<String>) -> Result<()> {
    let _storage = KeyringStorage::new(service);

    // Since keyring doesn't support listing, we'll need to check common networks
    // or read from a config file
    println!("Note: The keyring crate doesn't support listing all keys directly.");
    println!("To list keys, check your configuration file for configured networks.");
    println!("You can then use 'omikuji key export' to verify if a key exists for a network.");

    Ok(())
}

async fn remove_key(network: String, service: Option<String>) -> Result<()> {
    let storage = KeyringStorage::new(service);

    // Confirm removal
    println!(
        "Are you sure you want to remove the key for network '{}'? (y/N): ",
        network
    );
    let mut response = String::new();
    std::io::stdin().read_line(&mut response)?;

    if response.trim().to_lowercase() != "y" {
        println!("Key removal cancelled");
        return Ok(());
    }

    storage.remove_key(&network).await?;
    println!("Successfully removed key for network '{}'", network);

    Ok(())
}

async fn export_key(network: String, service: Option<String>) -> Result<()> {
    let storage = KeyringStorage::new(service);

    // Confirm export
    println!("WARNING: This will display your private key!");
    println!(
        "Are you sure you want to export the key for network '{}'? (y/N): ",
        network
    );
    let mut response = String::new();
    std::io::stdin().read_line(&mut response)?;

    if response.trim().to_lowercase() != "y" {
        println!("Key export cancelled");
        return Ok(());
    }

    let key = storage.get_key(&network).await?;
    println!(
        "Private key for network '{}': {}",
        network,
        secrecy::ExposeSecret::expose_secret(&key)
    );

    Ok(())
}

async fn migrate_keys(service: Option<String>) -> Result<()> {
    use crate::wallet::key_storage::{EnvVarStorage, KeyStorage};

    let env_storage = EnvVarStorage::new();
    let keyring_storage = KeyringStorage::new(service);

    let networks = env_storage.list_keys().await?;

    if networks.is_empty() {
        println!("No keys found in environment variables");
        return Ok(());
    }

    println!("Found keys for networks: {:?}", networks);
    println!("Migrating keys from environment variables to keyring...");

    for network in networks {
        match env_storage.get_key(&network).await {
            Ok(key) => match keyring_storage.store_key(&network, key).await {
                Ok(_) => println!("✓ Migrated key for network '{}'", network),
                Err(e) => println!("✗ Failed to migrate key for network '{}': {}", network, e),
            },
            Err(e) => println!("✗ Failed to read key for network '{}': {}", network, e),
        }
    }

    println!("\nMigration complete!");
    println!("You can now remove the private key environment variables.");

    Ok(())
}
