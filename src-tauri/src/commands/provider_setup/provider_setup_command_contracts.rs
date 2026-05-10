use std::fmt;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{commands::provider_contracts::GpuCloudProviderId, provider_setup};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GpuCloudProviderSetup {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub provider_user_email: String,
    pub provider_api_key_fingerprint: String,
}

impl From<provider_setup::GpuCloudProviderSetup> for GpuCloudProviderSetup {
    fn from(setup: provider_setup::GpuCloudProviderSetup) -> Self {
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

impl From<GetGpuCloudProviderSetupRequest> for provider_setup::GetGpuCloudProviderSetupRequest {
    fn from(request: GetGpuCloudProviderSetupRequest) -> Self {
        Self {
            gpu_cloud_provider_id: request.gpu_cloud_provider_id.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetGpuCloudProviderSetupResponse {
    pub gpu_cloud_provider_setup: Option<GpuCloudProviderSetup>,
}

impl From<provider_setup::GetGpuCloudProviderSetupResponse> for GetGpuCloudProviderSetupResponse {
    fn from(response: provider_setup::GetGpuCloudProviderSetupResponse) -> Self {
        Self {
            gpu_cloud_provider_setup: response.gpu_cloud_provider_setup.map(Into::into),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Type)]
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

impl From<SetupGpuCloudProviderRequest> for provider_setup::SetupGpuCloudProviderRequest {
    fn from(request: SetupGpuCloudProviderRequest) -> Self {
        Self {
            gpu_cloud_provider_id: request.gpu_cloud_provider_id.into(),
            provider_api_key: request.provider_api_key,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SetupGpuCloudProviderResponse {
    pub gpu_cloud_provider_setup: GpuCloudProviderSetup,
}

impl From<provider_setup::SetupGpuCloudProviderResponse> for SetupGpuCloudProviderResponse {
    fn from(response: provider_setup::SetupGpuCloudProviderResponse) -> Self {
        Self {
            gpu_cloud_provider_setup: response.gpu_cloud_provider_setup.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeleteGpuCloudProviderSetupRequest {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
}

impl From<DeleteGpuCloudProviderSetupRequest>
    for provider_setup::DeleteGpuCloudProviderSetupRequest
{
    fn from(request: DeleteGpuCloudProviderSetupRequest) -> Self {
        Self {
            gpu_cloud_provider_id: request.gpu_cloud_provider_id.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeleteGpuCloudProviderSetupResponse {
    pub gpu_cloud_provider_setup: Option<GpuCloudProviderSetup>,
}

impl From<provider_setup::DeleteGpuCloudProviderSetupResponse>
    for DeleteGpuCloudProviderSetupResponse
{
    fn from(response: provider_setup::DeleteGpuCloudProviderSetupResponse) -> Self {
        Self {
            gpu_cloud_provider_setup: response.gpu_cloud_provider_setup.map(Into::into),
        }
    }
}

#[cfg(test)]
#[path = "provider_setup_command_contract_tests.rs"]
mod provider_setup_command_contract_tests;
