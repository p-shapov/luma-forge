use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::provider_setup::ProviderSetupError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum GpuCloudProviderId {
    Runpod,
}

impl GpuCloudProviderId {
    pub fn keyring_account(&self) -> &'static str {
        match self {
            Self::Runpod => "runpod",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GpuCloudProviderSetup {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub provider_user_email: String,
    pub provider_api_key_fingerprint: String,
}

#[derive(Clone)]
pub struct ProviderApiKey(SecretString);

impl std::fmt::Debug for ProviderApiKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ProviderApiKey([REDACTED])")
    }
}

impl ProviderApiKey {
    pub fn new(value: String) -> Result<Self, ProviderSetupError> {
        if value.trim().is_empty() {
            return Err(ProviderSetupError::InvalidProviderApiKey);
        }

        Ok(Self(SecretString::from(value)))
    }

    pub fn expose_secret(&self) -> &str {
        self.0.expose_secret()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIdentity {
    pub provider_user_email: String,
    pub provider_api_key_fingerprint: String,
}
