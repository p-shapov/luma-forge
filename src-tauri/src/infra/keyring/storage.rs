use keyring::{Entry, Error as PlatformKeyringError};
use secrecy::{ExposeSecret, SecretString};

use super::KeyringStorageError;

#[derive(Clone)]
pub struct KeyringStorage {
    service_name: String,
}

impl KeyringStorage {
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
        }
    }

    pub async fn get(&self, account: &str) -> Result<Option<SecretString>, KeyringStorageError> {
        let service_name = self.service_name.clone();
        let account = account.to_owned();

        run_blocking(
            move || match entry(&service_name, &account)?.get_password() {
                Ok(secret) => Ok(Some(SecretString::from(secret))),
                Err(PlatformKeyringError::NoEntry) => Ok(None),
                Err(_) => Err(KeyringStorageError::Unavailable),
            },
        )
        .await
    }

    pub async fn set(
        &self,
        account: &str,
        secret: SecretString,
    ) -> Result<(), KeyringStorageError> {
        let service_name = self.service_name.clone();
        let account = account.to_owned();

        run_blocking(move || {
            entry(&service_name, &account)?
                .set_password(secret.expose_secret())
                .map_err(|_| KeyringStorageError::Unavailable)
        })
        .await
    }

    pub async fn delete(&self, account: &str) -> Result<(), KeyringStorageError> {
        let service_name = self.service_name.clone();
        let account = account.to_owned();

        run_blocking(
            move || match entry(&service_name, &account)?.delete_credential() {
                Ok(()) | Err(PlatformKeyringError::NoEntry) => Ok(()),
                Err(_) => Err(KeyringStorageError::Unavailable),
            },
        )
        .await
    }
}

fn entry(service_name: &str, account: &str) -> Result<Entry, KeyringStorageError> {
    Entry::new(service_name, account).map_err(|_| KeyringStorageError::Unavailable)
}

async fn run_blocking<T, F>(operation: F) -> Result<T, KeyringStorageError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, KeyringStorageError> + Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|_| KeyringStorageError::Unavailable)?
}
