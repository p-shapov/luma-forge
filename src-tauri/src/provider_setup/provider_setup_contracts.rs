use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    domain::provider_setup::GpuCloudProviderSetup as DomainGpuCloudProviderSetup,
    shared_contracts::provider_contracts::GpuCloudProviderId,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGpuCloudProviderSetupRequest {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGpuCloudProviderSetupResponse {
    pub gpu_cloud_provider_setup: Option<GpuCloudProviderSetup>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SetupGpuCloudProviderRequest {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub provider_api_key: String,
}

impl fmt::Debug for SetupGpuCloudProviderRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SetupGpuCloudProviderRequest")
            .field("gpu_cloud_provider_id", &self.gpu_cloud_provider_id)
            .field("provider_api_key", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupGpuCloudProviderResponse {
    pub gpu_cloud_provider_setup: GpuCloudProviderSetup,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteGpuCloudProviderSetupRequest {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteGpuCloudProviderSetupResponse {
    pub gpu_cloud_provider_setup: Option<GpuCloudProviderSetup>,
}
