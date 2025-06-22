use anyhow::Result;
use async_trait::async_trait;
use secrecy::SecretString;

pub mod env;
pub mod keyring;
#[cfg(test)]
mod tests;

pub use env::EnvVarStorage;
pub use keyring::KeyringStorage;

#[async_trait]
pub trait KeyStorage: Send + Sync {
    async fn get_key(&self, network: &str) -> Result<SecretString>;
    async fn store_key(&self, network: &str, key: SecretString) -> Result<()>;
    async fn remove_key(&self, network: &str) -> Result<()>;
    async fn list_keys(&self) -> Result<Vec<String>>;
}
