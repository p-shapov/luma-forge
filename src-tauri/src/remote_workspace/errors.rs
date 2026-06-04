use crate::domain::provider::GpuCloudProviderId;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteWorkspaceError {
    MissingProvider { provider_id: GpuCloudProviderId },
    InvalidRequest { message: String },
    NonExistingVolume,
    NonExistingProvisioner,
    NonExistingEndpoint,
    ProviderUnauthorized,
    ProviderRateLimited,
    ProviderTimeout,
    ProviderRequestFailed { message: String },
    ProvisionerWorkerTokenMissing,
    ProvisionerWorkerTokenInvalid,
    ProvisionerWorkerUnauthorized,
    ProvisionerWorkerUnavailable,
    ProvisionerWorkerConflict,
    ProvisionerWorkerResponseInvalid,
    ProvisionerWorkerFailed,
    ProvisionerWorkerAssetDownloadFailed,
    ProvisionerWorkerAssetAuthRequired,
    ProvisionerWorkerPathValidationFailed,
    ProvisionerWorkerStepTimeout,
    ProvisionerWorkerUnexpectedError,
    InvalidWorkspaceState { message: String },
    WorkspaceNotReady,
    MissingEndpoint,
    CleanupFailed { message: String },
    NotImplemented { message: String },
}
