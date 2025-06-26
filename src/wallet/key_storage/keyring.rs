use anyhow::{anyhow, Result};
use async_trait::async_trait;
use keyring::Entry;
use secrecy::{ExposeSecret, SecretString};
use tracing::{debug, error, info, warn};

use super::KeyStorage;

const DEFAULT_SERVICE: &str = "omikuji";

#[derive(Debug, Clone)]
pub struct KeyringStorage {
    service: String,
}

impl KeyringStorage {
    pub fn new(service: Option<String>) -> Self {
        Self {
            service: service.unwrap_or_else(|| DEFAULT_SERVICE.to_string()),
        }
    }

    fn get_entry(&self, network: &str) -> Result<Entry> {
        debug!(
            "Creating keyring entry with service: '{}', network: '{}'",
            self.service, network
        );

        Entry::new(&self.service, network).map_err(|e| {
            error!(
                "Failed to create keyring entry for service: '{}', network: '{}', error: {:?}",
                self.service, network, e
            );
            anyhow!("Failed to create keyring entry: {}", e)
        })
    }
}

#[async_trait]
impl KeyStorage for KeyringStorage {
    async fn get_key(&self, network: &str) -> Result<SecretString> {
        info!(
            "[KEYRING DEBUG] Starting key retrieval for network '{}' from keyring",
            network
        );
        info!("[KEYRING DEBUG] Service name: '{}'", self.service);

        // Check environment variables that might affect keyring
        debug!(
            "[KEYRING DEBUG] XDG_SESSION_TYPE: {:?}",
            std::env::var("XDG_SESSION_TYPE")
        );
        debug!("[KEYRING DEBUG] DISPLAY: {:?}", std::env::var("DISPLAY"));
        debug!(
            "[KEYRING DEBUG] SSH_CONNECTION: {:?}",
            std::env::var("SSH_CONNECTION")
        );
        debug!(
            "[KEYRING DEBUG] DBUS_SESSION_BUS_ADDRESS: {:?}",
            std::env::var("DBUS_SESSION_BUS_ADDRESS")
        );
        debug!("[KEYRING DEBUG] USER: {:?}", std::env::var("USER"));
        debug!("[KEYRING DEBUG] HOME: {:?}", std::env::var("HOME"));

        let entry = match self.get_entry(network) {
            Ok(e) => {
                info!("[KEYRING DEBUG] Successfully created keyring entry object");
                e
            }
            Err(e) => {
                error!("[KEYRING DEBUG] Failed to create keyring entry: {:?}", e);
                return Err(e);
            }
        };

        info!("[KEYRING DEBUG] Attempting to retrieve password from keyring");
        let password = match entry.get_password() {
            Ok(pwd) => {
                info!(
                    "[KEYRING DEBUG] Successfully retrieved password (length: {})",
                    pwd.len()
                );
                pwd
            }
            Err(e) => {
                error!("[KEYRING DEBUG] Failed to retrieve password from keyring");
                error!("[KEYRING DEBUG] Error type: {:?}", e);
                error!("[KEYRING DEBUG] Error message: {}", e);

                // Try to provide more specific error information
                let error_string = e.to_string();
                if error_string.contains("No such secret item")
                    || error_string.contains("not found")
                {
                    warn!(
                        "[KEYRING DEBUG] Key not found in keyring for network '{}'",
                        network
                    );
                } else if error_string.contains("D-Bus") || error_string.contains("dbus") {
                    warn!("[KEYRING DEBUG] D-Bus error - keyring service may not be accessible");
                    warn!("[KEYRING DEBUG] This often happens in headless/SSH sessions");
                } else if error_string.contains("Access denied")
                    || error_string.contains("Permission")
                {
                    warn!("[KEYRING DEBUG] Permission denied accessing keyring");
                }

                return Err(anyhow!(
                    "Failed to retrieve key for network '{}': {}",
                    network,
                    e
                ));
            }
        };

        info!(
            "[KEYRING DEBUG] Successfully retrieved key for network '{}'",
            network
        );
        Ok(SecretString::from(password))
    }

    async fn store_key(&self, network: &str, key: SecretString) -> Result<()> {
        info!(
            "[KEYRING DEBUG] Starting key storage for network '{}' in keyring",
            network
        );
        info!("[KEYRING DEBUG] Service name: '{}'", self.service);
        info!("[KEYRING DEBUG] Key length: {}", key.expose_secret().len());

        let entry = match self.get_entry(network) {
            Ok(e) => {
                info!("[KEYRING DEBUG] Successfully created keyring entry object for storage");
                e
            }
            Err(e) => {
                error!(
                    "[KEYRING DEBUG] Failed to create keyring entry for storage: {:?}",
                    e
                );
                return Err(e);
            }
        };

        info!("[KEYRING DEBUG] Attempting to store password in keyring");
        match entry.set_password(key.expose_secret()) {
            Ok(_) => {
                info!("[KEYRING DEBUG] Successfully stored password in keyring");
            }
            Err(e) => {
                error!("[KEYRING DEBUG] Failed to store password in keyring");
                error!("[KEYRING DEBUG] Error type: {:?}", e);
                error!("[KEYRING DEBUG] Error message: {}", e);

                let error_string = e.to_string();
                if error_string.contains("D-Bus") || error_string.contains("dbus") {
                    error!("[KEYRING DEBUG] D-Bus error - keyring service may not be accessible");
                    error!("[KEYRING DEBUG] This often happens in headless/SSH sessions");
                } else if error_string.contains("Access denied")
                    || error_string.contains("Permission")
                {
                    error!("[KEYRING DEBUG] Permission denied accessing keyring");
                }

                return Err(anyhow!(
                    "Failed to store key for network '{}': {}",
                    network,
                    e
                ));
            }
        }

        info!(
            "[KEYRING DEBUG] Successfully stored key for network '{}'",
            network
        );

        // Verify the key was stored by trying to retrieve it
        info!("[KEYRING DEBUG] Verifying stored key can be retrieved");
        match entry.get_password() {
            Ok(pwd) => {
                info!(
                    "[KEYRING DEBUG] Verification successful - retrieved password (length: {})",
                    pwd.len()
                );
            }
            Err(e) => {
                error!(
                    "[KEYRING DEBUG] Verification failed - could not retrieve just-stored key: {}",
                    e
                );
            }
        }

        Ok(())
    }

    async fn remove_key(&self, network: &str) -> Result<()> {
        info!("Removing key for network '{}' from keyring", network);

        let entry = self.get_entry(network)?;
        entry
            .delete_credential()
            .map_err(|e| anyhow!("Failed to remove key for network '{}': {}", network, e))?;

        info!("Successfully removed key for network '{}'", network);
        Ok(())
    }

    async fn list_keys(&self) -> Result<Vec<String>> {
        // Note: The keyring crate doesn't support listing all keys directly
        // This would need to be tracked separately or use platform-specific APIs
        Err(anyhow!(
            "Listing keys is not directly supported by the keyring crate. \
            Consider maintaining a separate list of networks in configuration."
        ))
    }
}
