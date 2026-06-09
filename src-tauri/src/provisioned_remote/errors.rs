use crate::domain::provider::ProviderApiError;

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
