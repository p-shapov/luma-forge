use keyring::{Entry, Error as KeyringError};

use crate::secrets_storage::{
    errors::SecretsStorageError,
    stores::{ApiSecret, SecretKey, SecretStore},
};

const KEYRING_SCOPE: &str = "secrets-storage";

#[derive(Debug, Clone)]
pub struct KeyringSecretStore {
    service_name: String,
}

impl KeyringSecretStore {
    pub fn new(app_identifier: impl AsRef<str>) -> Self {
        Self {
            service_name: format!("{}.{}", app_identifier.as_ref(), KEYRING_SCOPE),
        }
    }
}

async fn run_blocking_keyring_operation<T, F>(operation: F) -> Result<T, SecretsStorageError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, SecretsStorageError> + Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|_| SecretsStorageError::StoreUnavailable)?
}

fn keyring_entry(service_name: &str, key: SecretKey) -> Result<Entry, SecretsStorageError> {
    Entry::new(service_name, key.storage_account_name())
        .map_err(|_| SecretsStorageError::StoreUnavailable)
}

impl SecretStore for KeyringSecretStore {
    fn has<'a>(
        &'a self,
        key: SecretKey,
    ) -> crate::shared::AppFuture<'a, Result<bool, SecretsStorageError>> {
        let service_name = self.service_name.clone();

        Box::pin(async move {
            run_blocking_keyring_operation(move || {
                match keyring_entry(&service_name, key)?.get_password() {
                    Ok(_) => Ok(true),
                    Err(KeyringError::NoEntry) => Ok(false),
                    Err(_) => Err(SecretsStorageError::StoreUnavailable),
                }
            })
            .await
        })
    }

    fn write<'a>(
        &'a self,
        key: SecretKey,
        secret: ApiSecret,
    ) -> crate::shared::AppFuture<'a, Result<(), SecretsStorageError>> {
        let service_name = self.service_name.clone();

        Box::pin(async move {
            run_blocking_keyring_operation(move || {
                keyring_entry(&service_name, key)?
                    .set_password(secret.expose_secret())
                    .map_err(|_| SecretsStorageError::StoreUnavailable)
            })
            .await
        })
    }

    fn delete<'a>(
        &'a self,
        key: SecretKey,
    ) -> crate::shared::AppFuture<'a, Result<(), SecretsStorageError>> {
        let service_name = self.service_name.clone();

        Box::pin(async move {
            run_blocking_keyring_operation(move || {
                match keyring_entry(&service_name, key)?.delete_credential() {
                    Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
                    Err(_) => Err(SecretsStorageError::StoreUnavailable),
                }
            })
            .await
        })
    }

    fn read<'a>(
        &'a self,
        key: SecretKey,
    ) -> crate::shared::AppFuture<'a, Result<Option<ApiSecret>, SecretsStorageError>> {
        let service_name = self.service_name.clone();

        Box::pin(async move {
            run_blocking_keyring_operation(move || {
                match keyring_entry(&service_name, key)?.get_password() {
                    Ok(secret) => ApiSecret::new(secret)
                        .map(Some)
                        .map_err(|_| SecretsStorageError::StoredSecretInvalid),
                    Err(KeyringError::NoEntry) => Ok(None),
                    Err(_) => Err(SecretsStorageError::StoreUnavailable),
                }
            })
            .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyring_entry_uses_secret_key_storage_account_name() {
        assert_eq!(SecretKey::RunpodApiKey.storage_account_name(), "runpod");
        assert_eq!(
            SecretKey::HuggingFaceApiKey.storage_account_name(),
            "hugging-face"
        );
    }

    #[test]
    fn service_name_uses_app_identifier_and_scope() {
        let store = KeyringSecretStore::new("com.luma-forge.dev");

        assert_eq!(store.service_name, "com.luma-forge.dev.secrets-storage");
    }
}
