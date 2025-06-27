use anyhow::{anyhow, Result};
use async_trait::async_trait;
use keyring::Entry;
use secrecy::{ExposeSecret, SecretString};
use std::time::Duration;
use tokio::time::sleep;
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
            "[KEYRING DEBUG] Creating keyring entry with service: '{}', network: '{}'",
            self.service, network
        );
        
        // Log the exact parameters that will be used by the keyring crate
        debug!(
            "[KEYRING DEBUG] Entry::new parameters - service: '{}' (len: {}), username: '{}' (len: {})",
            self.service,
            self.service.len(),
            network,
            network.len()
        );
        
        // Check for any special characters or whitespace
        if self.service.contains(char::is_whitespace) {
            warn!("[KEYRING DEBUG] Service name contains whitespace characters");
        }
        if network.contains(char::is_whitespace) {
            warn!("[KEYRING DEBUG] Network name contains whitespace characters");
        }
        
        // Log if names contain any non-ASCII characters
        if !self.service.is_ascii() {
            warn!("[KEYRING DEBUG] Service name contains non-ASCII characters");
        }
        if !network.is_ascii() {
            warn!("[KEYRING DEBUG] Network name contains non-ASCII characters");
        }

        Entry::new(&self.service, network).map_err(|e| {
            error!(
                "[KEYRING DEBUG] Failed to create keyring entry for service: '{}', network: '{}', error: {:?}",
                self.service, network, e
            );
            error!("[KEYRING DEBUG] Error details: {}", e);
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
        
        // macOS-specific checks
        #[cfg(target_os = "macos")]
        {
            debug!(
                "[KEYRING DEBUG] macOS Security Framework - TMPDIR: {:?}",
                std::env::var("TMPDIR")
            );
            debug!(
                "[KEYRING DEBUG] macOS - checking keychain access (this is macOS)",
            );
        }

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
        info!("[KEYRING DEBUG] Network/username: '{}'", network);
        info!("[KEYRING DEBUG] Key length: {}", key.expose_secret().len());

        // Check environment variables that might affect keyring behavior
        info!("[KEYRING DEBUG] Checking environment for keyring-affecting variables:");
        debug!(
            "[KEYRING DEBUG] KEYRING_BACKEND: {:?}",
            std::env::var("KEYRING_BACKEND")
        );
        debug!(
            "[KEYRING DEBUG] PYTHON_KEYRING_BACKEND: {:?}",
            std::env::var("PYTHON_KEYRING_BACKEND")
        );
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
        debug!(
            "[KEYRING DEBUG] XDG_RUNTIME_DIR: {:?}",
            std::env::var("XDG_RUNTIME_DIR")
        );

        // Check if we're in a container environment
        info!("[KEYRING DEBUG] Checking for container environment:");
        if std::path::Path::new("/.dockerenv").exists() {
            warn!("[KEYRING DEBUG] Running inside Docker container (/.dockerenv exists)");
        }
        if std::env::var("KUBERNETES_SERVICE_HOST").is_ok() {
            warn!("[KEYRING DEBUG] Running inside Kubernetes container");
        }
        if let Ok(cgroup) = std::fs::read_to_string("/proc/1/cgroup") {
            if cgroup.contains("/docker/") || cgroup.contains("/kubepods/") {
                warn!("[KEYRING DEBUG] Container detected via /proc/1/cgroup");
            }
        }

        // Check OS and platform information
        debug!("[KEYRING DEBUG] OS: {}", std::env::consts::OS);
        debug!("[KEYRING DEBUG] ARCH: {}", std::env::consts::ARCH);
        debug!(
            "[KEYRING DEBUG] Current working directory: {:?}",
            std::env::current_dir()
        );
        
        // macOS-specific keychain checks
        #[cfg(target_os = "macos")]
        {
            info!("[KEYRING DEBUG] Running on macOS - using macOS Keychain");
            debug!(
                "[KEYRING DEBUG] macOS TMPDIR: {:?}",
                std::env::var("TMPDIR")
            );
            // Check if we're in a sandboxed environment
            if let Ok(app_sandbox) = std::env::var("APP_SANDBOX_CONTAINER_ID") {
                warn!("[KEYRING DEBUG] Running in macOS sandbox: {}", app_sandbox);
            }
        }

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

        // Show exact parameters being used
        info!(
            "[KEYRING DEBUG] Keyring entry parameters - Service: '{}', Username/Network: '{}'",
            self.service, network
        );

        info!("[KEYRING DEBUG] Attempting to store password in keyring");
        match entry.set_password(key.expose_secret()) {
            Ok(_) => {
                info!("[KEYRING DEBUG] set_password() returned Ok - password should be stored");
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

        // Immediate verification
        info!("[KEYRING DEBUG] Performing immediate verification of stored key");
        match entry.get_password() {
            Ok(pwd) => {
                info!(
                    "[KEYRING DEBUG] Immediate verification successful - retrieved password (length: {})",
                    pwd.len()
                );
                if pwd.len() != key.expose_secret().len() {
                    error!(
                        "[KEYRING DEBUG] WARNING: Retrieved password length ({}) differs from stored length ({})",
                        pwd.len(),
                        key.expose_secret().len()
                    );
                }
            }
            Err(e) => {
                error!(
                    "[KEYRING DEBUG] Immediate verification failed - could not retrieve just-stored key: {}",
                    e
                );
            }
        }

        // Add a small delay and re-verify to check for timing issues
        info!("[KEYRING DEBUG] Waiting 100ms before re-verification to check for timing issues");
        sleep(Duration::from_millis(100)).await;

        // Create a new entry to ensure fresh retrieval
        let verify_entry = match self.get_entry(network) {
            Ok(e) => e,
            Err(e) => {
                error!(
                    "[KEYRING DEBUG] Failed to create new entry for delayed verification: {:?}",
                    e
                );
                return Ok(()); // Still return Ok since the initial store succeeded
            }
        };

        info!("[KEYRING DEBUG] Performing delayed verification with fresh entry");
        match verify_entry.get_password() {
            Ok(pwd) => {
                info!(
                    "[KEYRING DEBUG] Delayed verification successful - retrieved password (length: {})",
                    pwd.len()
                );
                if pwd == key.expose_secret() {
                    info!("[KEYRING DEBUG] Password content matches exactly");
                } else {
                    error!("[KEYRING DEBUG] WARNING: Retrieved password content differs from stored!");
                }
            }
            Err(e) => {
                error!(
                    "[KEYRING DEBUG] Delayed verification failed - could not retrieve key after delay: {}",
                    e
                );
                warn!("[KEYRING DEBUG] This suggests a persistence issue with the keyring backend");
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
