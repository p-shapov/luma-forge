use keyring::{Entry, Error as KeyringError};

use crate::domain::provider_setup::{GpuCloudProviderId, ProviderApiKey};

use super::SecretStoreError;

const GPU_CLOUD_PROVIDER_KEYRING_SCOPE: &str = "gpu-cloud-provider";

pub trait SecretStore: Send + Sync {
    fn has_api_key_entry(&self, provider_id: &GpuCloudProviderId)
        -> Result<bool, SecretStoreError>;

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

#[derive(Debug, Clone)]
pub struct KeyringSecretStore {
    service_name: String,
}

impl KeyringSecretStore {
    pub fn new(app_identifier: impl AsRef<str>) -> Self {
        Self {
            service_name: format!(
                "{}.{GPU_CLOUD_PROVIDER_KEYRING_SCOPE}",
                app_identifier.as_ref()
            ),
        }
    }

    fn entry(&self, provider_id: &GpuCloudProviderId) -> Result<Entry, SecretStoreError> {
        Entry::new(&self.service_name, keyring_account(provider_id))
            .map_err(|_| SecretStoreError::SecureKeyringUnavailable)
    }
}

fn keyring_account(provider_id: &GpuCloudProviderId) -> &'static str {
    match provider_id {
        GpuCloudProviderId::Runpod => "runpod",
    }
}

impl SecretStore for KeyringSecretStore {
    fn has_api_key_entry(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> Result<bool, SecretStoreError> {
        match self.entry(provider_id)?.get_password() {
            Ok(_) => Ok(true),
            Err(KeyringError::NoEntry) => Ok(false),
            Err(_) => Err(SecretStoreError::SecureKeyringUnavailable),
        }
    }

    fn read_api_key(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> Result<Option<ProviderApiKey>, SecretStoreError> {
        match self.entry(provider_id)?.get_password() {
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
        self.entry(provider_id)?
            .set_password(api_key.expose_secret())
            .map_err(|_| SecretStoreError::SecureKeyringUnavailable)
    }

    fn delete_api_key(&self, provider_id: &GpuCloudProviderId) -> Result<(), SecretStoreError> {
        self.entry(provider_id)?
            .delete_credential()
            .map_err(|_| SecretStoreError::SecureKeyringUnavailable)
    }
}
