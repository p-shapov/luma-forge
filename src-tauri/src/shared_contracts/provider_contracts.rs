use serde::{Deserialize, Serialize};

use crate::domain::provider_setup::GpuCloudProviderId as DomainGpuCloudProviderId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
