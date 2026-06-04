use crate::domain::provider::GpuCloudProviderId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderApiError {
    Unauthorized,
    RateLimited,
    Timeout,
    RequestFailed { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateVolumeError {
    ExistingVolume,
    ProviderApi(ProviderApiError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteVolumeError {
    NonExistingVolume,
    ProviderApi(ProviderApiError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObserveVolumeError {
    ProviderApi(ProviderApiError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartProvisionerError {
    ExistingProvisioner,
    ProviderApi(ProviderApiError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminateProvisionerError {
    NonExistingProvisioner,
    ProviderApi(ProviderApiError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObserveProvisionerError {
    ProviderApi(ProviderApiError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GetProvisionerStatusError {
    ProviderApi(ProviderApiError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateEndpointError {
    ExistingEndpoint,
    ProviderApi(ProviderApiError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteEndpointError {
    NonExistingEndpoint,
    ProviderApi(ProviderApiError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObserveEndpointError {
    ProviderApi(ProviderApiError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteWorkspaceProviderRegistryError {
    MissingProvider { provider_id: GpuCloudProviderId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteWorkspaceError {
    MissingProvider { provider_id: GpuCloudProviderId },
    InvalidRequest { message: String },
    ExistingVolume,
    ExistingProvisioner,
    ExistingEndpoint,
    ProviderApi(ProviderApiError),
    InvalidWorkspaceState { message: String },
    WorkspaceNotReady,
    MissingEndpoint,
    CleanupFailed { message: String },
    NotImplemented { message: String },
}

pub type WorkspaceSetupError = RemoteWorkspaceError;
pub type WorkspaceObserveError = RemoteWorkspaceError;
pub type WorkspaceProvisionError = RemoteWorkspaceError;
pub type WorkspaceExecuteError = RemoteWorkspaceError;
pub type WorkspaceDeleteError = RemoteWorkspaceError;
