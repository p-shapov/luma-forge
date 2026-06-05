use crate::domain::{
    provider::{GpuCloudProviderId, ProviderError},
    workspace::RemoteProvisioningError,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteWorkspaceError {
    SetupWorkspaceInvalidRequest { message: String },
    ProviderUnavailable { provider_id: GpuCloudProviderId },
    Provider(ProviderError),
    RemoteVolumeNotFound,
    RemoteProvisionerNotFound,
    RemoteEndpointNotFound,
    ExecuteWorkspaceNotReady,
    ExecuteWorkspaceMissingEndpoint,
    ExecuteWorkspaceNotImplemented { message: String },
    DeleteWorkspaceFailed { message: String },
}

impl From<RemoteWorkspaceError> for RemoteProvisioningError {
    fn from(error: RemoteWorkspaceError) -> Self {
        match error {
            RemoteWorkspaceError::Provider(error) => RemoteProvisioningError::Provider(error),
            error => RemoteProvisioningError::InvalidProvisioningState {
                message: format!("{error:?}"),
            },
        }
    }
}
