use crate::domain::{
    provisioned_remote::ProviderApiError, provisioned_remote::ProvisionedRemoteLifecycleError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvisionedRemoteError {
    WorkspaceNotFound,
    WorkspaceAlreadyExists,
    LifecycleOperationAlreadyRunning { workspace_id: String },
    ProviderAdapterUnavailable,
    ProviderSecretUnavailable,
    ProviderApiFailed(ProviderApiError),
    ProvisionerUnavailable,
    ProvisionerResponseInvalid,
    ProvisionerFailed,
    RemoteVolumeNotFound,
    RemoteProvisionerNotFound,
    RemoteEndpointNotFound,
    InvalidRuntimeState,
    StorageUnavailable,
}

impl From<ProviderApiError> for ProvisionedRemoteError {
    fn from(error: ProviderApiError) -> Self {
        Self::ProviderApiFailed(error)
    }
}

impl From<ProvisionedRemoteLifecycleError> for ProvisionedRemoteError {
    fn from(error: ProvisionedRemoteLifecycleError) -> Self {
        match error {
            ProvisionedRemoteLifecycleError::AppInterrupted => Self::InvalidRuntimeState,
            ProvisionedRemoteLifecycleError::ProviderAdapterUnavailable => {
                Self::ProviderAdapterUnavailable
            }
            ProvisionedRemoteLifecycleError::ProviderSecretUnavailable => {
                Self::ProviderSecretUnavailable
            }
            ProvisionedRemoteLifecycleError::ProviderApiFailed { reason } => {
                Self::ProviderApiFailed(reason)
            }
            ProvisionedRemoteLifecycleError::ProvisionerUnavailable => Self::ProvisionerUnavailable,
            ProvisionedRemoteLifecycleError::ProvisionerResponseInvalid => {
                Self::ProvisionerResponseInvalid
            }
            ProvisionedRemoteLifecycleError::ProvisionerFailed => Self::ProvisionerFailed,
            ProvisionedRemoteLifecycleError::RemoteVolumeNotFound => Self::RemoteVolumeNotFound,
            ProvisionedRemoteLifecycleError::RemoteProvisionerNotFound => {
                Self::RemoteProvisionerNotFound
            }
            ProvisionedRemoteLifecycleError::RemoteEndpointNotFound => Self::RemoteEndpointNotFound,
            ProvisionedRemoteLifecycleError::InvalidRuntimeState => Self::InvalidRuntimeState,
        }
    }
}
