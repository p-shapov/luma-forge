use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "lowercase")]
pub(crate) enum GpuCloudProviderId {
    #[serde(rename = "runpod")]
    RunPod,
}

impl GpuCloudProviderId {
    pub(crate) fn parse(value: &str) -> Result<Self, ProviderSetupError> {
        match value {
            "runpod" => Ok(Self::RunPod),
            _ => Err(ProviderSetupError::UnsupportedProvider),
        }
    }

    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::RunPod => "runpod",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub(crate) struct GpuCloudProviderSetup {
    pub(crate) gpu_cloud_provider_id: GpuCloudProviderId,
    pub(crate) provider_user_id: String,
    pub(crate) provider_api_key_fingerprint: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProviderSetupMetadata {
    pub(crate) provider_id: GpuCloudProviderId,
    pub(crate) provider_user_id: String,
    pub(crate) provider_api_key_fingerprint: String,
}

impl ProviderSetupMetadata {
    pub(crate) fn redacted_setup(&self) -> GpuCloudProviderSetup {
        GpuCloudProviderSetup {
            gpu_cloud_provider_id: self.provider_id.clone(),
            provider_user_id: self.provider_user_id.clone(),
            provider_api_key_fingerprint: self.provider_api_key_fingerprint.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ValidatedProviderCredential {
    pub(crate) provider_user_id: String,
    pub(crate) provider_api_key_fingerprint: String,
}

#[derive(Clone, Debug, thiserror::Error)]
pub(crate) enum ProviderSetupError {
    #[error("unsupported provider")]
    UnsupportedProvider,
    #[error("provider setup is incomplete")]
    ProviderSetupIncomplete,
    #[error("provider setup already exists")]
    ProviderSetupAlreadyExists,
    #[error("invalid provider api key")]
    InvalidProviderApiKey,
    #[error("provider api is unavailable")]
    ProviderApiUnavailable,
    #[error("secure keyring is unavailable")]
    SecureKeyringUnavailable,
    #[error("local storage is unavailable")]
    LocalStorageUnavailable,
}
