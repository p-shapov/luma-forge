use keyring::{Entry, Error as KeyringError};

use crate::domain::provider_setup::{GpuCloudProviderId, ProviderApiKey};

use super::SecretStoreError;

const KEYRING_SERVICE: &str = "com.pavelshapov.luma-forge.gpu-cloud-provider";

pub trait SecretStore: Send + Sync {
    fn read_api_key(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> Result<Option<ProviderApiKey>, SecretStoreError>;

    fn replace_api_key(
        &self,
        provider_id: &GpuCloudProviderId,
        api_key: &ProviderApiKey,
    ) -> Result<(), SecretStoreError>;

    fn delete_api_key(&self, provider_id: &GpuCloudProviderId) -> Result<(), SecretStoreError>;
}

#[derive(Debug, Clone, Default)]
pub struct KeyringSecretStore;

impl KeyringSecretStore {
    fn entry(provider_id: &GpuCloudProviderId) -> Result<Entry, SecretStoreError> {
        Entry::new(KEYRING_SERVICE, keyring_account(provider_id))
            .map_err(|_| SecretStoreError::SecureKeyringUnavailable)
    }
}

fn keyring_account(provider_id: &GpuCloudProviderId) -> &'static str {
    match provider_id {
        GpuCloudProviderId::Runpod => "runpod",
    }
}

impl SecretStore for KeyringSecretStore {
    fn read_api_key(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> Result<Option<ProviderApiKey>, SecretStoreError> {
        match Self::entry(provider_id)?.get_password() {
            Ok(api_key) => ProviderApiKey::new(api_key)
                .map(Some)
                .map_err(|_| SecretStoreError::InvalidStoredProviderApiKey),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(_) => Err(SecretStoreError::SecureKeyringUnavailable),
        }
    }

    fn replace_api_key(
        &self,
        provider_id: &GpuCloudProviderId,
        api_key: &ProviderApiKey,
    ) -> Result<(), SecretStoreError> {
        Self::entry(provider_id)?
            .set_password(api_key.expose_secret())
            .map_err(|_| SecretStoreError::SecureKeyringUnavailable)
    }

    fn delete_api_key(&self, provider_id: &GpuCloudProviderId) -> Result<(), SecretStoreError> {
        Self::entry(provider_id)?
            .delete_credential()
            .map_err(|_| SecretStoreError::SecureKeyringUnavailable)
    }
}
