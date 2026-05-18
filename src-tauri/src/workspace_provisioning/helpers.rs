use thiserror::Error;

use crate::{
    domain::workspace::{
        provisioning_state::progress_for_workspace, PersistentStorageVolumeSnapshot,
        ProvisioningPodSnapshot, ServerlessEndpointSnapshot, Workspace,
        WorkspaceProvisioningProgress,
    },
    secrets::SecretStoreError,
    workspace_provisioner::ProvisionerWorkerError,
    workspace_resources::{
        NetworkVolumeObservation, ProvisioningPodObservation, ServerlessEndpointObservation,
        WorkspaceResourceError,
    },
};

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum WorkspaceProvisioningError {
    #[error("workspace not found")]
    WorkspaceNotFound,
    #[error("invalid workspace lifecycle")]
    InvalidWorkspaceLifecycle,
    #[error("workspace catalog unavailable")]
    WorkspaceCatalogUnavailable,
    #[error("provider setup is incomplete")]
    ProviderSetupIncomplete,
    #[error("provider api key unauthorized")]
    ProviderApiKeyUnauthorized,
    #[error("provider api unavailable")]
    ProviderApiUnavailable,
    #[error("provider rate limited")]
    ProviderRateLimited,
    #[error("provider request rejected")]
    ProviderRequestRejected,
    #[error("provider response invalid")]
    ProviderResponseInvalid,
    #[error("provider resource not found")]
    ProviderResourceNotFound,
    #[error("provider operation conflict")]
    ProviderOperationConflict,
    #[error("provider operation indeterminate")]
    ProviderOperationIndeterminate,
    #[error("secure keyring unavailable")]
    SecureKeyringUnavailable,
    #[error("provisioner worker token invalid")]
    ProvisionerWorkerTokenInvalid,
    #[error("provisioner worker unauthorized")]
    ProvisionerWorkerUnauthorized,
    #[error("provisioner worker unavailable")]
    ProvisionerWorkerUnavailable,
    #[error("provisioner worker conflict")]
    ProvisionerWorkerConflict,
    #[error("provisioner worker response invalid")]
    ProvisionerWorkerResponseInvalid { diagnostic: Option<String> },
    #[error("provisioner worker failed")]
    ProvisionerWorkerFailed { diagnostic: Option<String> },
}

impl From<SecretStoreError> for WorkspaceProvisioningError {
    fn from(error: SecretStoreError) -> Self {
        match error {
            SecretStoreError::SecureKeyringUnavailable => Self::SecureKeyringUnavailable,
            SecretStoreError::InvalidStoredProviderApiKey => Self::ProviderSetupIncomplete,
            SecretStoreError::InvalidStoredProvisionerWorkerToken => {
                Self::ProvisionerWorkerTokenInvalid
            }
        }
    }
}

impl From<WorkspaceResourceError> for WorkspaceProvisioningError {
    fn from(error: WorkspaceResourceError) -> Self {
        match error {
            WorkspaceResourceError::WorkspaceCatalogUnavailable => {
                Self::WorkspaceCatalogUnavailable
            }
            WorkspaceResourceError::ProviderSetupIncomplete => Self::ProviderSetupIncomplete,
            WorkspaceResourceError::ProviderApiKeyUnauthorized => Self::ProviderApiKeyUnauthorized,
            WorkspaceResourceError::ProviderApiUnavailable => Self::ProviderApiUnavailable,
            WorkspaceResourceError::ProviderRateLimited => Self::ProviderRateLimited,
            WorkspaceResourceError::ProviderRequestRejected => Self::ProviderRequestRejected,
            WorkspaceResourceError::ProviderResponseInvalid => Self::ProviderResponseInvalid,
            WorkspaceResourceError::ProviderResourceNotFound => Self::ProviderResourceNotFound,
            WorkspaceResourceError::ProviderOperationConflict => Self::ProviderOperationConflict,
            WorkspaceResourceError::ProviderOperationIndeterminate => {
                Self::ProviderOperationIndeterminate
            }
            WorkspaceResourceError::SecureKeyringUnavailable => Self::SecureKeyringUnavailable,
            WorkspaceResourceError::ProvisionerWorkerTokenInvalid => {
                Self::ProvisionerWorkerTokenInvalid
            }
        }
    }
}

impl From<ProvisionerWorkerError> for WorkspaceProvisioningError {
    fn from(error: ProvisionerWorkerError) -> Self {
        match error {
            ProvisionerWorkerError::Unauthorized => Self::ProvisionerWorkerUnauthorized,
            ProvisionerWorkerError::Conflict => Self::ProvisionerWorkerConflict,
            ProvisionerWorkerError::Unreachable => Self::ProvisionerWorkerUnavailable,
            ProvisionerWorkerError::InvalidPayload { diagnostic } => {
                Self::ProvisionerWorkerResponseInvalid { diagnostic }
            }
            ProvisionerWorkerError::TerminalFailure { diagnostic } => {
                Self::ProvisionerWorkerFailed { diagnostic }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceProvisioningResult {
    pub workspace: Workspace,
    pub progress: WorkspaceProvisioningProgress,
}

pub(crate) fn result(workspace: Workspace) -> WorkspaceProvisioningResult {
    let progress = progress_for_workspace(&workspace);
    WorkspaceProvisioningResult {
        workspace,
        progress,
    }
}

pub(crate) fn persistent_storage_volume_snapshot(
    workspace: &Workspace,
    observation: NetworkVolumeObservation,
) -> PersistentStorageVolumeSnapshot {
    PersistentStorageVolumeSnapshot {
        gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
        provider_resource_id: observation.provider_resource_id,
        provider_resource_status: observation.provider_resource_status,
        mount_path: observation.mount_path,
    }
}

pub(crate) fn observed_provisioning_pod_snapshot(
    workspace: &Workspace,
    previous: &ProvisioningPodSnapshot,
    observation: ProvisioningPodObservation,
) -> ProvisioningPodSnapshot {
    ProvisioningPodSnapshot {
        gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
        provider_resource_id: observation.provider_resource_id,
        provider_resource_status: observation.provider_resource_status,
        provisioner_status_url: observation
            .provisioner_status_url
            .unwrap_or_else(|| previous.provisioner_status_url.clone()),
    }
}

pub(crate) fn serverless_endpoint_snapshot(
    workspace: &Workspace,
    observation: ServerlessEndpointObservation,
) -> ServerlessEndpointSnapshot {
    ServerlessEndpointSnapshot {
        gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
        provider_resource_id: observation.provider_resource_id,
        provider_resource_status: observation.provider_resource_status,
        endpoint_invoke_url: observation.endpoint_invoke_url,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_provisioning_token_secret_failures_to_provisioning_errors() {
        assert_eq!(
            WorkspaceProvisioningError::from(SecretStoreError::InvalidStoredProvisionerWorkerToken),
            WorkspaceProvisioningError::ProvisionerWorkerTokenInvalid
        );
        assert_eq!(
            WorkspaceProvisioningError::from(SecretStoreError::SecureKeyringUnavailable),
            WorkspaceProvisioningError::SecureKeyringUnavailable
        );
    }
}
