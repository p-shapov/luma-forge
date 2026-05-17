use std::{future::Future, pin::Pin};

use crate::domain::provider_setup::GpuCloudProviderId;

use super::{
    contracts::{
        CreateEndpointTemplateInput, CreateNetworkVolumeInput, CreateProvisioningPodInput,
        CreateServerlessEndpointInput, DiscoverEndpointTemplatesInput, DiscoverNetworkVolumesInput,
        DiscoverProvisioningPodsInput, DiscoverServerlessEndpointsInput,
        EndpointTemplateObservation, NetworkVolumeObservation, ObserveProvisioningPodInput,
        ProvisioningPodObservation, ServerlessEndpointObservation,
    },
    ProviderResourceError,
};

pub trait ProviderResourceGateway: Send + Sync {
    fn create_network_volume<'a>(
        &'a self,
        input: CreateNetworkVolumeInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<NetworkVolumeObservation, ProviderResourceError>>
                + Send
                + 'a,
        >,
    >;

    fn get_network_volume<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        volume_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<NetworkVolumeObservation, ProviderResourceError>>
                + Send
                + 'a,
        >,
    >;

    fn discover_network_volumes<'a>(
        &'a self,
        input: DiscoverNetworkVolumesInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<NetworkVolumeObservation>, ProviderResourceError>>
                + Send
                + 'a,
        >,
    >;

    fn delete_network_volume<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        volume_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProviderResourceError>> + Send + 'a>>;

    fn create_provisioning_pod<'a>(
        &'a self,
        input: CreateProvisioningPodInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisioningPodObservation, ProviderResourceError>>
                + Send
                + 'a,
        >,
    >;

    fn discover_provisioning_pods<'a>(
        &'a self,
        input: DiscoverProvisioningPodsInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<ProvisioningPodObservation>, ProviderResourceError>>
                + Send
                + 'a,
        >,
    >;

    fn get_provisioning_pod<'a>(
        &'a self,
        input: ObserveProvisioningPodInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisioningPodObservation, ProviderResourceError>>
                + Send
                + 'a,
        >,
    >;

    fn delete_provisioning_pod<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        pod_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProviderResourceError>> + Send + 'a>>;

    fn create_endpoint_template<'a>(
        &'a self,
        input: CreateEndpointTemplateInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<EndpointTemplateObservation, ProviderResourceError>>
                + Send
                + 'a,
        >,
    >;

    fn discover_endpoint_templates<'a>(
        &'a self,
        input: DiscoverEndpointTemplatesInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<EndpointTemplateObservation>, ProviderResourceError>>
                + Send
                + 'a,
        >,
    >;

    fn get_endpoint_template<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        template_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<EndpointTemplateObservation, ProviderResourceError>>
                + Send
                + 'a,
        >,
    >;

    fn delete_endpoint_template<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        template_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProviderResourceError>> + Send + 'a>>;

    fn create_serverless_endpoint<'a>(
        &'a self,
        input: CreateServerlessEndpointInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ServerlessEndpointObservation, ProviderResourceError>>
                + Send
                + 'a,
        >,
    >;

    fn discover_serverless_endpoints<'a>(
        &'a self,
        input: DiscoverServerlessEndpointsInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<ServerlessEndpointObservation>, ProviderResourceError>>
                + Send
                + 'a,
        >,
    >;

    fn get_serverless_endpoint<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        endpoint_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ServerlessEndpointObservation, ProviderResourceError>>
                + Send
                + 'a,
        >,
    >;

    fn delete_serverless_endpoint<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        endpoint_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProviderResourceError>> + Send + 'a>>;
}
