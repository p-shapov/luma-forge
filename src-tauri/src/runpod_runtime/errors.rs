use serde::{Deserialize, Serialize};

use crate::{
    domain::runpod::{RunpodLifecycleError, RunpodProvisionerError, RunpodRuntimeStateError},
    secrets_storage::SecretsStorageError,
    shared::ApiError,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderApiError {
    Unauthorized,
    InsufficientPermissions,
    RateLimited,
    Timeout,
    RequestFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunpodRuntimeError {
    WorkspaceNotFound,
    WorkspaceAlreadyExists,
    LifecycleOperationAlreadyRunning { workspace_id: String },
    RunpodSecretUnavailable,
    RunpodApiFailed(ProviderApiError),
    ProvisionerUnavailable,
    ProvisionerResponseInvalid,
    ProvisionerFailed,
    NetworkVolumeNotFound,
    ProvisionerPodNotFound,
    EndpointNotFound,
    TemplateNotFound,
    InvalidRuntimeState,
    StorageUnavailable,
}

impl From<ProviderApiError> for RunpodRuntimeError {
    fn from(error: ProviderApiError) -> Self {
        Self::RunpodApiFailed(error)
    }
}

impl From<ProviderApiError> for ApiError {
    fn from(error: ProviderApiError) -> Self {
        match error {
            ProviderApiError::Unauthorized => Self::Unauthorized,
            ProviderApiError::InsufficientPermissions => Self::InsufficientPermissions,
            ProviderApiError::RateLimited => Self::RateLimited,
            ProviderApiError::Timeout => Self::Timeout,
            ProviderApiError::RequestFailed => Self::RequestFailed {
                message: "RunPod request failed".to_string(),
            },
        }
    }
}

impl From<ApiError> for ProviderApiError {
    fn from(error: ApiError) -> Self {
        match error {
            ApiError::Unauthorized => Self::Unauthorized,
            ApiError::InsufficientPermissions => Self::InsufficientPermissions,
            ApiError::RateLimited => Self::RateLimited,
            ApiError::Timeout => Self::Timeout,
            ApiError::RequestFailed { .. } => Self::RequestFailed,
        }
    }
}

impl From<RunpodLifecycleError> for RunpodRuntimeError {
    fn from(error: RunpodLifecycleError) -> Self {
        match error {
            RunpodLifecycleError::AppInterrupted => Self::InvalidRuntimeState,
            RunpodLifecycleError::RunPodSecretError(SecretsStorageError::KeyNotFound) => {
                Self::RunpodSecretUnavailable
            }
            RunpodLifecycleError::RunPodSecretError(_) => Self::StorageUnavailable,
            RunpodLifecycleError::RunPodApiError(reason) => Self::RunpodApiFailed(reason.into()),
            RunpodLifecycleError::ProvisionerError(RunpodProvisionerError::Unavailable) => {
                Self::ProvisionerUnavailable
            }
            RunpodLifecycleError::ProvisionerError(RunpodProvisionerError::ResponseInvalid) => {
                Self::ProvisionerResponseInvalid
            }
            RunpodLifecycleError::ProvisionerError(RunpodProvisionerError::Failed) => {
                Self::ProvisionerFailed
            }
            RunpodLifecycleError::InvalidRuntimeState(RunpodRuntimeStateError::MissingVolume) => {
                Self::NetworkVolumeNotFound
            }
            RunpodLifecycleError::InvalidRuntimeState(RunpodRuntimeStateError::MissingEndpoint) => {
                Self::EndpointNotFound
            }
            RunpodLifecycleError::InvalidRuntimeState(RunpodRuntimeStateError::MissingTemplate) => {
                Self::TemplateNotFound
            }
            RunpodLifecycleError::InvalidRuntimeState(
                RunpodRuntimeStateError::MissingProvisionerPod,
            ) => Self::ProvisionerPodNotFound,
            RunpodLifecycleError::InvalidRuntimeState(RunpodRuntimeStateError::Invalid) => {
                Self::InvalidRuntimeState
            }
        }
    }
}
