use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::provider_setup as domain_provider_setup;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GpuCloudProviderId {
    Runpod,
}

impl From<GpuCloudProviderId> for domain_provider_setup::GpuCloudProviderId {
    fn from(provider_id: GpuCloudProviderId) -> Self {
        match provider_id {
            GpuCloudProviderId::Runpod => domain_provider_setup::GpuCloudProviderId::Runpod,
        }
    }
}

impl From<domain_provider_setup::GpuCloudProviderId> for GpuCloudProviderId {
    fn from(provider_id: domain_provider_setup::GpuCloudProviderId) -> Self {
        match provider_id {
            domain_provider_setup::GpuCloudProviderId::Runpod => Self::Runpod,
        }
    }
}
