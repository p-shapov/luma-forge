use serde::{Deserialize, Serialize};

use super::placement::RunpodPlacementPlan;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunpodRuntime {
    pub placement: RunpodPlacementPlan,
    pub resources: RunpodResources,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunpodResources {
    pub network_volume_id: Option<String>,
    pub provisioner_pod_id: Option<String>,
    pub endpoint_id: Option<String>,
    pub template_id: Option<String>,
}

impl RunpodResources {
    pub fn is_empty(&self) -> bool {
        self.network_volume_id.is_none()
            && self.provisioner_pod_id.is_none()
            && self.endpoint_id.is_none()
            && self.template_id.is_none()
    }
}
