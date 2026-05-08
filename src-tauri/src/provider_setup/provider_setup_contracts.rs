use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::provider_setup::{
    GpuCloudProviderId as DomainGpuCloudProviderId,
    GpuCloudProviderSetup as DomainGpuCloudProviderSetup,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum GpuCloudProviderId {
    Runpod,
}

impl From<GpuCloudProviderId> for DomainGpuCloudProviderId {
    fn from(provider_id: GpuCloudProviderId) -> Self {
        match provider_id {
            GpuCloudProviderId::Runpod => Self::Runpod,
        }
    }
}

impl From<DomainGpuCloudProviderId> for GpuCloudProviderId {
    fn from(provider_id: DomainGpuCloudProviderId) -> Self {
        match provider_id {
            DomainGpuCloudProviderId::Runpod => Self::Runpod,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GpuCloudProviderSetup {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub provider_user_email: String,
    pub provider_api_key_fingerprint: String,
}

impl From<DomainGpuCloudProviderSetup> for GpuCloudProviderSetup {
    fn from(setup: DomainGpuCloudProviderSetup) -> Self {
        Self {
            gpu_cloud_provider_id: setup.gpu_cloud_provider_id.into(),
            provider_user_email: setup.provider_user_email,
            provider_api_key_fingerprint: setup.provider_api_key_fingerprint,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetGpuCloudProviderSetupRequest {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetGpuCloudProviderSetupResponse {
    pub gpu_cloud_provider_setup: Option<GpuCloudProviderSetup>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SetupGpuCloudProviderRequest {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub provider_api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SetupGpuCloudProviderResponse {
    pub gpu_cloud_provider_setup: GpuCloudProviderSetup,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeleteGpuCloudProviderSetupRequest {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeleteGpuCloudProviderSetupResponse {
    pub gpu_cloud_provider_setup: Option<GpuCloudProviderSetup>,
}
