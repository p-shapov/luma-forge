use crate::domain::{runpod_runtime::ProviderApiError, runpod_runtime::RunpodLifecycleError};

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

impl From<RunpodLifecycleError> for RunpodRuntimeError {
    fn from(error: RunpodLifecycleError) -> Self {
        match error {
            RunpodLifecycleError::AppInterrupted => Self::InvalidRuntimeState,
            RunpodLifecycleError::RunpodSecretUnavailable => Self::RunpodSecretUnavailable,
            RunpodLifecycleError::RunpodApiFailed { reason } => Self::RunpodApiFailed(reason),
            RunpodLifecycleError::ProvisionerUnavailable => Self::ProvisionerUnavailable,
            RunpodLifecycleError::ProvisionerResponseInvalid => Self::ProvisionerResponseInvalid,
            RunpodLifecycleError::ProvisionerFailed => Self::ProvisionerFailed,
            RunpodLifecycleError::NetworkVolumeNotFound => Self::NetworkVolumeNotFound,
            RunpodLifecycleError::ProvisionerPodNotFound => Self::ProvisionerPodNotFound,
            RunpodLifecycleError::EndpointNotFound => Self::EndpointNotFound,
            RunpodLifecycleError::TemplateNotFound => Self::TemplateNotFound,
            RunpodLifecycleError::InvalidRuntimeState => Self::InvalidRuntimeState,
        }
    }
}
