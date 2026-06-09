use serde::{Deserialize, Serialize};

use super::{
    placement::RemotePlacementPlan,
    provider::{GpuCloudProviderId, ProviderApiError},
};

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
    pub volume: Option<ProvisionedRemoteVolumeSnapshot>,
    pub provisioner: Option<ProvisionedRemoteProvisionerSnapshot>,
    pub endpoint: Option<ProvisionedRemoteEndpointSnapshot>,
}

impl ProvisionedRemoteResources {
    pub fn is_empty(&self) -> bool {
        self.volume.is_none() && self.provisioner.is_none() && self.endpoint.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionedRemoteVolumeSnapshot {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionedRemoteProvisionerSnapshot {
    pub id: String,
    pub status_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionedRemoteEndpointSnapshot {
    pub id: String,
    pub url: String,
    pub template_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionedRemoteProvisionerStatus {
    Pending,
    Starting,
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionedRemoteLifecycleError {
    AppInterrupted,
    ProviderAdapterUnavailable,
    ProviderSecretUnavailable,
    ProviderApiFailed { reason: ProviderApiError },
    ProvisionerUnavailable,
    ProvisionerResponseInvalid,
    ProvisionerFailed,
    RemoteVolumeNotFound,
    RemoteProvisionerNotFound,
    RemoteEndpointNotFound,
    InvalidRuntimeState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionedRemoteProvisionStep {
    CreateVolume,
    StartProvisioner,
    PollProvisioner,
    TerminateProvisioner,
    CreateEndpoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionedRemoteCleanupStep {
    DeleteEndpoint,
    TerminateProvisioner,
    DeleteVolume,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionedRemoteDeleteStep {
    DeleteEndpoint,
    TerminateProvisioner,
    DeleteVolume,
    DeleteLocalWorkspace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum ProvisionedRemoteLifecycleOperationPayload {
    Provision {
        step: Option<ProvisionedRemoteProvisionStep>,
        error: Option<ProvisionedRemoteLifecycleError>,
    },
    Cleanup {
        step: Option<ProvisionedRemoteCleanupStep>,
        error: Option<ProvisionedRemoteLifecycleError>,
    },
    Delete {
        step: Option<ProvisionedRemoteDeleteStep>,
        error: Option<ProvisionedRemoteLifecycleError>,
    },
}
