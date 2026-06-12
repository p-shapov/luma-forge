use serde::{Deserialize, Serialize};

use super::provider::ProviderApiError;

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
    CreateTemplate,
    CreateEndpoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionedRemoteCleanupStep {
    DeleteEndpoint,
    DeleteTemplate,
    TerminateProvisioner,
    DeleteVolume,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionedRemoteDeleteStep {
    DeleteEndpoint,
    DeleteTemplate,
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
