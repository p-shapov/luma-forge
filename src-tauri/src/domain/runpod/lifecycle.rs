use serde::{Deserialize, Serialize};

use crate::shared::ApiError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum RunpodProvisionerError {
    #[error("provisioner unavailable: {message}")]
    Unavailable { message: String },
    #[error("provisioner response invalid: {message}")]
    ResponseInvalid { message: String },
    #[error("provisioner failed: {message}")]
    Failed { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum RunpodRuntimeStateError {
    #[error("runtime state invalid: {message}")]
    Invalid { message: String },
    #[error("runtime state missing volume")]
    MissingVolume,
    #[error("runtime state missing endpoint")]
    MissingEndpoint,
    #[error("runtime state missing template")]
    MissingTemplate,
    #[error("runtime state missing provisioner pod")]
    MissingProvisionerPod,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum RunpodLifecycleError {
    #[error("app interrupted")]
    AppInterrupted,
    #[error("runpod api error")]
    RunPodApiError(#[from] ApiError),
    #[error("runpod provisioner error")]
    ProvisionerError(#[from] RunpodProvisionerError),
    #[error("runpod runtime state error")]
    InvalidRuntimeState(#[from] RunpodRuntimeStateError),
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
