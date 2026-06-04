use crate::domain::provider::GpuCloudProviderId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteWorkspaceError {
    MissingProvider { provider_id: GpuCloudProviderId },
    InvalidRequest { message: String },
    ExistingVolume,
    NonExistingVolume,
    ExistingProvisioner,
    NonExistingProvisioner,
    ExistingEndpoint,
    NonExistingEndpoint,
    ProviderUnauthorized,
    ProviderRateLimited,
    ProviderTimeout,
    ProviderRequestFailed { message: String },
    InvalidWorkspaceState { message: String },
    WorkspaceNotReady,
    MissingEndpoint,
    CleanupFailed { message: String },
    NotImplemented { message: String },
}
