use keyring::{Entry, Error as KeyringError};
use secrecy::{ExposeSecret, SecretString};

use crate::domain::provider_setup::{GpuCloudProviderId, ProviderApiKey};

use super::SecretStoreError;

const GPU_CLOUD_PROVIDER_KEYRING_SCOPE: &str = "gpu-cloud-provider";
const PROVISIONER_WORKER_KEYRING_SCOPE: &str = "provisioner-worker";

#[derive(Clone)]
pub struct ProvisionerWorkerBearerToken(SecretString);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionerWorkerBearerTokenError;

impl std::fmt::Debug for ProvisionerWorkerBearerToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ProvisionerWorkerBearerToken([REDACTED])")
    }
}

impl ProvisionerWorkerBearerToken {
    pub fn new(value: String) -> Result<Self, ProvisionerWorkerBearerTokenError> {
        if value.trim().is_empty() {
            return Err(ProvisionerWorkerBearerTokenError);
        }

        Ok(Self(SecretString::from(value)))
    }

    pub fn expose_secret(&self) -> &str {
        self.0.expose_secret()
    }
}

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

    fn write_provisioner_worker_token(
        &self,
        workspace_id: &str,
        token: &ProvisionerWorkerBearerToken,
    ) -> Result<(), SecretStoreError>;

    fn read_provisioner_worker_token(
        &self,
        workspace_id: &str,
    ) -> Result<Option<ProvisionerWorkerBearerToken>, SecretStoreError>;

    fn delete_provisioner_worker_token(&self, workspace_id: &str) -> Result<(), SecretStoreError>;
}

#[derive(Debug, Clone)]
pub struct KeyringSecretStore {
    provider_service_name: String,
    provisioner_worker_service_name: String,
}

impl KeyringSecretStore {
    pub fn new(app_identifier: impl AsRef<str>) -> Self {
        Self {
            provider_service_name: format!(
                "{}.{GPU_CLOUD_PROVIDER_KEYRING_SCOPE}",
                app_identifier.as_ref()
            ),
            provisioner_worker_service_name: format!(
                "{}.{PROVISIONER_WORKER_KEYRING_SCOPE}",
                app_identifier.as_ref()
            ),
        }
    }

    fn provider_api_key_entry(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> Result<Entry, SecretStoreError> {
        Entry::new(&self.provider_service_name, keyring_account(provider_id))
            .map_err(|_| SecretStoreError::SecureKeyringUnavailable)
    }

    fn provisioner_worker_entry(&self, workspace_id: &str) -> Result<Entry, SecretStoreError> {
        Entry::new(
            &self.provisioner_worker_service_name,
            &provisioner_worker_account(workspace_id),
        )
        .map_err(|_| SecretStoreError::SecureKeyringUnavailable)
    }
}

fn keyring_account(provider_id: &GpuCloudProviderId) -> &'static str {
    match provider_id {
        GpuCloudProviderId::Runpod => "runpod",
    }
}

fn provisioner_worker_account(workspace_id: &str) -> String {
    format!("workspace:{workspace_id}")
}

impl SecretStore for KeyringSecretStore {
    fn has_api_key_entry(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> Result<bool, SecretStoreError> {
        match self.provider_api_key_entry(provider_id)?.get_password() {
            Ok(_) => Ok(true),
            Err(KeyringError::NoEntry) => Ok(false),
            Err(_) => Err(SecretStoreError::SecureKeyringUnavailable),
        }
    }

    fn read_api_key(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> Result<Option<ProviderApiKey>, SecretStoreError> {
        match self.provider_api_key_entry(provider_id)?.get_password() {
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
        self.provider_api_key_entry(provider_id)?
            .set_password(api_key.expose_secret())
            .map_err(|_| SecretStoreError::SecureKeyringUnavailable)
    }

    fn delete_api_key(&self, provider_id: &GpuCloudProviderId) -> Result<(), SecretStoreError> {
        self.provider_api_key_entry(provider_id)?
            .delete_credential()
            .map_err(|_| SecretStoreError::SecureKeyringUnavailable)
    }

    fn write_provisioner_worker_token(
        &self,
        workspace_id: &str,
        token: &ProvisionerWorkerBearerToken,
    ) -> Result<(), SecretStoreError> {
        self.provisioner_worker_entry(workspace_id)?
            .set_password(token.expose_secret())
            .map_err(|_| SecretStoreError::SecureKeyringUnavailable)
    }

    fn read_provisioner_worker_token(
        &self,
        workspace_id: &str,
    ) -> Result<Option<ProvisionerWorkerBearerToken>, SecretStoreError> {
        match self.provisioner_worker_entry(workspace_id)?.get_password() {
            Ok(token) => ProvisionerWorkerBearerToken::new(token)
                .map(Some)
                .map_err(|_| SecretStoreError::InvalidStoredProvisionerWorkerToken),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(_) => Err(SecretStoreError::SecureKeyringUnavailable),
        }
    }

    fn delete_provisioner_worker_token(&self, workspace_id: &str) -> Result<(), SecretStoreError> {
        match self
            .provisioner_worker_entry(workspace_id)?
            .delete_credential()
        {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(_) => Err(SecretStoreError::SecureKeyringUnavailable),
        }
    }
}
