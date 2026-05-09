use secrecy::{ExposeSecret, SecretString};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuCloudProviderId {
    Runpod,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIdentity {
    pub provider_user_email: String,
    pub provider_api_key_fingerprint: String,
}
