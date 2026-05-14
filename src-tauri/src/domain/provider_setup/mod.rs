use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

pub mod validator;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GpuCloudProviderId {
    Runpod,
}

impl GpuCloudProviderId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Runpod => "runpod",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuCloudProviderSetup {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub provider_user_email: String,
    pub provider_api_key_fingerprint: String,
}

#[derive(Clone)]
pub struct ProviderApiKey(SecretString);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderApiKeyError;

impl std::fmt::Debug for ProviderApiKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ProviderApiKey([REDACTED])")
    }
}

impl ProviderApiKey {
    pub fn new(value: String) -> Result<Self, ProviderApiKeyError> {
        if value.trim().is_empty() {
            return Err(ProviderApiKeyError);
        }

        Ok(Self(SecretString::from(value)))
    }

    pub fn expose_secret(&self) -> &str {
        self.0.expose_secret()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderIdentity {
    pub provider_user_email: String,
    pub provider_api_key_fingerprint: String,
}
