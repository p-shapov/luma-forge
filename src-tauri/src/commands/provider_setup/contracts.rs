use std::fmt;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::provider_setup as domain_provider_setup;

// Command-boundary metadata only. These remote definitions provide generated
// binding shapes for domain types without making domain modules depend on Specta.
#[allow(dead_code)]
mod remote_types {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, Type)]
    #[specta(remote = domain_provider_setup::GpuCloudProviderSetup)]
    pub(super) struct GpuCloudProviderSetup {
        pub gpu_cloud_provider_id: domain_provider_setup::GpuCloudProviderId,
        pub provider_user_email: String,
        pub provider_api_key_fingerprint: String,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetGpuCloudProviderSetupRequest {
    pub gpu_cloud_provider_id: domain_provider_setup::GpuCloudProviderId,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetGpuCloudProviderSetupResponse {
    pub gpu_cloud_provider_setup: Option<domain_provider_setup::GpuCloudProviderSetup>,
}

#[derive(Clone, Serialize, Deserialize, Type)]
pub struct SetupGpuCloudProviderRequest {
    pub gpu_cloud_provider_id: domain_provider_setup::GpuCloudProviderId,
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

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SetupGpuCloudProviderResponse {
    pub gpu_cloud_provider_setup: domain_provider_setup::GpuCloudProviderSetup,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeleteGpuCloudProviderSetupRequest {
    pub gpu_cloud_provider_id: domain_provider_setup::GpuCloudProviderId,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeleteGpuCloudProviderSetupResponse {
    pub gpu_cloud_provider_setup: Option<domain_provider_setup::GpuCloudProviderSetup>,
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
