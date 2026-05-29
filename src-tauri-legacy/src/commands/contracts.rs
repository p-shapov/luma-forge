use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::provider_setup as domain_provider_setup;

// Command-boundary metadata only. These remote definitions provide generated
// binding shapes for domain types without making domain modules depend on Specta.
#[allow(dead_code)]
mod remote_types {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
    #[specta(remote = domain_provider_setup::GpuCloudProviderId)]
    #[serde(rename_all = "snake_case")]
    pub(super) enum GpuCloudProviderId {
        Runpod,
    }
}
