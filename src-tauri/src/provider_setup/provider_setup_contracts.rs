use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    domain::provider_setup::GpuCloudProviderSetup as DomainGpuCloudProviderSetup,
    shared_contracts::provider_contracts::GpuCloudProviderId,
};

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
