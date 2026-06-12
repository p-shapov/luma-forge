use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum GpuCloudProviderIdDto {
    #[serde(rename = "runpod")]
    Runpod,
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
}
