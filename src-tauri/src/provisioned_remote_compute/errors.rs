use crate::domain::{
    provider::{GpuCloudProviderId, ProviderApiError},
    workspace::ProvisionedRemoteComputeProvisioningError,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionedRemoteComputeError {
    SetupWorkspaceInvalidRequest { message: String },
    ProviderUnavailable { provider_id: GpuCloudProviderId },
    ProviderSecretUnavailable,
    ProvisioningAlreadyRunning { workspace_id: String },
    Provider(ProviderApiError),
    RemoteVolumeNotFound,
    RemoteProvisionerNotFound,
    RemoteEndpointNotFound,
    ProvisionerWorker(ProvisionedRemoteComputeProvisioningError),
    ExecuteWorkspaceNotReady,
    ExecuteWorkspaceMissingEndpoint,
    ExecuteWorkspaceNotImplemented { message: String },
    DeleteWorkspaceFailed { message: String },
}

impl From<ProviderApiError> for ProvisionedRemoteComputeError {
    fn from(error: ProviderApiError) -> Self {
        Self::Provider(error)
    }
}

impl From<ProvisionedRemoteComputeError> for ProvisionedRemoteComputeProvisioningError {
    fn from(error: ProvisionedRemoteComputeError) -> Self {
        match error {
            ProvisionedRemoteComputeError::Provider(error) => {
                ProvisionedRemoteComputeProvisioningError::Provider(error)
            }
            ProvisionedRemoteComputeError::ProvisionerWorker(error) => error,
            error => ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                message: format!("{error:?}"),
            },
        }
    }
}
