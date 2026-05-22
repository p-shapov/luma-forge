mod context;
mod contracts;
mod error;
mod naming;
mod providers;
mod service;
pub(crate) mod state;

pub(crate) use context::WorkspaceResourceContext;
pub(crate) use contracts::{
    CreateNetworkVolumeInput, CreateProvisioningPodInput, CreateServerlessEndpointInput,
    DiscoverNetworkVolumesInput, DiscoverProvisioningPodsInput, DiscoverServerlessEndpointsInput,
    NetworkVolumeObservation, ObserveProvisioningPodInput, ProvisioningPodObservation,
    ServerlessEndpointObservation,
};
pub(crate) use error::WorkspaceResourceError;
pub(crate) use naming::provider_resource_name;
pub(crate) use providers::{WorkspaceResourceProviderRegistry, WorkspaceResourceProviderResolver};
pub(crate) use service::{WorkspaceResourceService, WorkspaceResourceSyncResult};
