use serde::{Deserialize, Serialize};

use super::provider::ProviderApiError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunpodProvisionerStatus {
    Pending,
    Starting,
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunpodLifecycleError {
    AppInterrupted,
    RunpodSecretUnavailable,
    RunpodApiFailed { reason: ProviderApiError },
    ProvisionerUnavailable,
    ProvisionerResponseInvalid,
    ProvisionerFailed,
    NetworkVolumeNotFound,
    ProvisionerPodNotFound,
    EndpointNotFound,
    TemplateNotFound,
    InvalidRuntimeState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunpodProvisionStep {
    CreateNetworkVolume,
    StartProvisionerPod,
    PollProvisioner,
    TerminateProvisionerPod,
    CreateTemplate,
    CreateEndpoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunpodCleanupStep {
    DeleteEndpoint,
    DeleteTemplate,
    TerminateProvisionerPod,
    DeleteNetworkVolume,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunpodDeleteStep {
    DeleteEndpoint,
    DeleteTemplate,
    TerminateProvisionerPod,
    DeleteNetworkVolume,
    DeleteLocalWorkspace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum RunpodLifecycleOperationPayload {
    Provision {
        step: Option<RunpodProvisionStep>,
        error: Option<RunpodLifecycleError>,
    },
    Cleanup {
        step: Option<RunpodCleanupStep>,
        error: Option<RunpodLifecycleError>,
    },
    Delete {
        step: Option<RunpodDeleteStep>,
        error: Option<RunpodLifecycleError>,
    },
}
