mod contracts;
mod error;
mod naming;
pub(crate) mod operations;

pub(crate) use contracts::{
    CreateEndpointTemplateInput, CreateNetworkVolumeInput, CreateProvisioningPodInput,
    CreateServerlessEndpointInput, DiscoverEndpointTemplatesInput, DiscoverNetworkVolumesInput,
    DiscoverProvisioningPodsInput, DiscoverServerlessEndpointsInput, EndpointTemplateObservation,
    NetworkVolumeObservation, ObserveProvisioningPodInput, ProvisioningPodObservation,
    ServerlessEndpointObservation,
};
pub(crate) use error::WorkspaceResourceError;
pub(crate) use naming::provider_resource_name;
