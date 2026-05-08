use keyring::{Entry, Error as KeyringError};

use crate::{
    domain::provider_setup::{GpuCloudProviderId, ProviderApiKey},
    provider_setup::ProviderSetupError,
};

const KEYRING_SERVICE: &str = "com.pavelshapov.luma-forge.gpu-cloud-provider";

pub trait SecretStore: Send + Sync {
    fn read_api_key(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> Result<Option<ProviderApiKey>, ProviderSetupError>;

    fn replace_api_key(
        &self,
        provider_id: &GpuCloudProviderId,
        api_key: &ProviderApiKey,
    ) -> Result<(), ProviderSetupError>;

    fn delete_api_key(&self, provider_id: &GpuCloudProviderId) -> Result<(), ProviderSetupError>;
}

#[derive(Debug, Clone, Default)]
pub struct KeyringSecretStore;

impl KeyringSecretStore {
    fn entry(provider_id: &GpuCloudProviderId) -> Result<Entry, ProviderSetupError> {
        Entry::new(KEYRING_SERVICE, provider_id.keyring_account())
            .map_err(|_| ProviderSetupError::SecureKeyringUnavailable)
    }
}

impl SecretStore for KeyringSecretStore {
    fn read_api_key(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> Result<Option<ProviderApiKey>, ProviderSetupError> {
        match Self::entry(provider_id)?.get_password() {
            Ok(api_key) => ProviderApiKey::new(api_key).map(Some),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(_) => Err(ProviderSetupError::SecureKeyringUnavailable),
        }
    }

    fn replace_api_key(
        &self,
        provider_id: &GpuCloudProviderId,
        api_key: &ProviderApiKey,
    ) -> Result<(), ProviderSetupError> {
        Self::entry(provider_id)?
            .set_password(api_key.expose_secret())
            .map_err(|_| ProviderSetupError::SecureKeyringUnavailable)
    }

    fn delete_api_key(&self, provider_id: &GpuCloudProviderId) -> Result<(), ProviderSetupError> {
        Self::entry(provider_id)?
            .delete_credential()
            .map_err(|_| ProviderSetupError::SecureKeyringUnavailable)
    }
}
