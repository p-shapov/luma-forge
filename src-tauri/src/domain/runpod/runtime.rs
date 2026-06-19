use serde::{Deserialize, Serialize};

use super::placement::RunpodPlacementPlan;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunpodRuntime {
    pub placement: RunpodPlacementPlan,
    pub resources: RunpodResources,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunpodResources {
    pub network_volume_id: Option<String>,
    pub provisioner_pod_id: Option<String>,
    pub endpoint_id: Option<String>,
    pub template_id: Option<String>,
}
