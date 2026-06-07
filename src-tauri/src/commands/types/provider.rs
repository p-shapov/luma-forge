use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::provider::GpuCloudProviderId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum GpuCloudProviderIdDto {
    #[serde(rename = "runpod")]
    Runpod,
}

impl From<GpuCloudProviderId> for GpuCloudProviderIdDto {
    fn from(value: GpuCloudProviderId) -> Self {
        match value {
            GpuCloudProviderId::Runpod => Self::Runpod,
        }
    }
}

impl From<GpuCloudProviderIdDto> for GpuCloudProviderId {
    fn from(value: GpuCloudProviderIdDto) -> Self {
        match value {
            GpuCloudProviderIdDto::Runpod => Self::Runpod,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_id_dto_serializes_stable_value() {
        assert_eq!(
            serde_json::to_string(&GpuCloudProviderIdDto::Runpod).expect("provider json"),
            "\"runpod\""
        );
    }

    #[test]
    fn provider_id_mapping_is_exhaustive_for_current_domain() {
        assert_eq!(
            GpuCloudProviderIdDto::from(GpuCloudProviderId::Runpod),
            GpuCloudProviderIdDto::Runpod
        );
        assert_eq!(
            GpuCloudProviderId::from(GpuCloudProviderIdDto::Runpod),
            GpuCloudProviderId::Runpod
        );
    }
}
