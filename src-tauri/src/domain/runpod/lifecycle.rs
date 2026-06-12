use serde::{Deserialize, Serialize};

use crate::{secrets_storage::SecretsStorageError, shared::ApiError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunpodProvisionerError {
    Unavailable,
    ResponseInvalid,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunpodRuntimeStateError {
    Invalid,
    MissingVolume,
    MissingEndpoint,
    MissingTemplate,
    MissingProvisionerPod,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunpodLifecycleError {
    AppInterrupted,
    RunPodSecretError(SecretsStorageError),
    RunPodApiError(ApiError),
    ProvisionerError(RunpodProvisionerError),
    InvalidRuntimeState(RunpodRuntimeStateError),
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
