use serde::{Deserialize, Serialize};

use super::{placement::RemotePlacementPlan, provider::GpuCloudProviderId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionedRemoteRuntime {
    pub placement: RemotePlacementPlan,
    pub resources: ProvisionedRemoteResources,
}

impl ProvisionedRemoteRuntime {
    pub fn provider_id(&self) -> GpuCloudProviderId {
        self.placement.gpu_cloud_provider_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionedRemoteResources {
    pub volume_id: Option<String>,
    pub provisioner_id: Option<String>,
    pub endpoint_id: Option<String>,
}

impl ProvisionedRemoteResources {
    pub fn is_empty(&self) -> bool {
        self.volume_id.is_none() && self.provisioner_id.is_none() && self.endpoint_id.is_none()
    }
}
