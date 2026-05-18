mod context;
mod contracts;
mod error;
mod naming;
pub(crate) mod operations;
mod service;

pub(crate) use context::WorkspaceResourceContext;
pub(crate) use contracts::{
    CreateEndpointTemplateInput, CreateNetworkVolumeInput, CreateProvisioningPodInput,
    CreateServerlessEndpointInput, DiscoverEndpointTemplatesInput, DiscoverNetworkVolumesInput,
    DiscoverProvisioningPodsInput, DiscoverServerlessEndpointsInput, EndpointTemplateObservation,
    NetworkVolumeObservation, ObserveProvisioningPodInput, ProvisioningPodObservation,
    ServerlessEndpointObservation,
};
pub(crate) use error::WorkspaceResourceError;
pub(crate) use naming::provider_resource_name;
pub(crate) use service::{
    WorkspaceResourceConfig, WorkspaceResourceService, WorkspaceResourceSyncResult,
};
