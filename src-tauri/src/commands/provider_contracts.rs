use serde::{Deserialize, Serialize};
use specta::Type;

use crate::shared_contracts::provider_contracts as application_contracts;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GpuCloudProviderId {
    Runpod,
}

impl From<GpuCloudProviderId> for application_contracts::GpuCloudProviderId {
    fn from(provider_id: GpuCloudProviderId) -> Self {
        match provider_id {
            GpuCloudProviderId::Runpod => Self::Runpod,
        }
    }
}

impl From<application_contracts::GpuCloudProviderId> for GpuCloudProviderId {
    fn from(provider_id: application_contracts::GpuCloudProviderId) -> Self {
        match provider_id {
            application_contracts::GpuCloudProviderId::Runpod => Self::Runpod,
        }
    }
}
