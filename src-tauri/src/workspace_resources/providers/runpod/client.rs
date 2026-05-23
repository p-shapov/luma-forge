use std::{future::Future, pin::Pin};

use crate::{
    domain::provider_setup::ProviderApiKey,
    provider::{
        runpod::{
            RunPodClient, RunPodCreateEndpointRequest, RunPodCreateNetworkVolumeRequest,
            RunPodCreatePodRequest, RunPodCreateTemplateRequest, RunPodEndpointObservation,
            RunPodNetworkVolumeObservation, RunPodPodObservation, RunPodTemplateObservation,
        },
        ProviderClientError,
    },
};

pub(super) trait RunPodWorkspaceResourceClient: Send + Sync {
    fn create_network_volume<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        request: &'a RunPodCreateNetworkVolumeRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<RunPodNetworkVolumeObservation, ProviderClientError>>
                + Send
                + 'a,
        >,
    >;

    fn get_network_volume<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        volume_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<RunPodNetworkVolumeObservation, ProviderClientError>>
                + Send
                + 'a,
        >,
    >;

    fn find_network_volumes_by_name<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        name: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<RunPodNetworkVolumeObservation>, ProviderClientError>>
                + Send
                + 'a,
        >,
    >;

    fn delete_network_volume<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        volume_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProviderClientError>> + Send + 'a>>;

    fn create_pod<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        request: &'a RunPodCreatePodRequest,
    ) -> Pin<Box<dyn Future<Output = Result<RunPodPodObservation, ProviderClientError>> + Send + 'a>>;

    fn get_pod<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        pod_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<RunPodPodObservation, ProviderClientError>> + Send + 'a>>;

    fn find_pods_by_name<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        name: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<RunPodPodObservation>, ProviderClientError>> + Send + 'a,
        >,
    >;

    fn delete_pod<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        pod_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProviderClientError>> + Send + 'a>>;

    fn create_template<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        request: &'a RunPodCreateTemplateRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<RunPodTemplateObservation, ProviderClientError>> + Send + 'a,
        >,
    >;

    fn find_templates_by_name<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        name: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<RunPodTemplateObservation>, ProviderClientError>>
                + Send
                + 'a,
        >,
    >;

    fn delete_template<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        template_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProviderClientError>> + Send + 'a>>;

    fn create_endpoint<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        request: &'a RunPodCreateEndpointRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<RunPodEndpointObservation, ProviderClientError>> + Send + 'a,
        >,
    >;

    fn get_endpoint<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        endpoint_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<RunPodEndpointObservation, ProviderClientError>> + Send + 'a,
        >,
    >;

    fn find_endpoints_by_name<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        name: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<RunPodEndpointObservation>, ProviderClientError>>
                + Send
                + 'a,
        >,
    >;

    fn delete_endpoint<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        endpoint_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProviderClientError>> + Send + 'a>>;
}

impl RunPodWorkspaceResourceClient for RunPodClient {
    fn create_network_volume<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        request: &'a RunPodCreateNetworkVolumeRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<RunPodNetworkVolumeObservation, ProviderClientError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move { RunPodClient::create_network_volume(self, api_key, request).await })
    }

    fn get_network_volume<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        volume_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<RunPodNetworkVolumeObservation, ProviderClientError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move { RunPodClient::get_network_volume(self, api_key, volume_id).await })
    }

    fn find_network_volumes_by_name<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        name: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<RunPodNetworkVolumeObservation>, ProviderClientError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(
            async move { RunPodClient::find_network_volumes_by_name(self, api_key, name).await },
        )
    }

    fn delete_network_volume<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        volume_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProviderClientError>> + Send + 'a>> {
        Box::pin(async move { RunPodClient::delete_network_volume(self, api_key, volume_id).await })
    }

    fn create_pod<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        request: &'a RunPodCreatePodRequest,
    ) -> Pin<Box<dyn Future<Output = Result<RunPodPodObservation, ProviderClientError>> + Send + 'a>>
    {
        Box::pin(async move { RunPodClient::create_pod(self, api_key, request).await })
    }

    fn get_pod<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        pod_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<RunPodPodObservation, ProviderClientError>> + Send + 'a>>
    {
        Box::pin(async move { RunPodClient::get_pod(self, api_key, pod_id).await })
    }

    fn find_pods_by_name<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        name: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<RunPodPodObservation>, ProviderClientError>> + Send + 'a,
        >,
    > {
        Box::pin(async move { RunPodClient::find_pods_by_name(self, api_key, name).await })
    }

    fn delete_pod<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        pod_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProviderClientError>> + Send + 'a>> {
        Box::pin(async move { RunPodClient::delete_pod(self, api_key, pod_id).await })
    }

    fn create_template<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        request: &'a RunPodCreateTemplateRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<RunPodTemplateObservation, ProviderClientError>> + Send + 'a,
        >,
    > {
        Box::pin(async move { RunPodClient::create_template(self, api_key, request).await })
    }

    fn find_templates_by_name<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        name: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<RunPodTemplateObservation>, ProviderClientError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move { RunPodClient::find_templates_by_name(self, api_key, name).await })
    }

    fn delete_template<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        template_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProviderClientError>> + Send + 'a>> {
        Box::pin(async move { RunPodClient::delete_template(self, api_key, template_id).await })
    }

    fn create_endpoint<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        request: &'a RunPodCreateEndpointRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<RunPodEndpointObservation, ProviderClientError>> + Send + 'a,
        >,
    > {
        Box::pin(async move { RunPodClient::create_endpoint(self, api_key, request).await })
    }

    fn get_endpoint<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        endpoint_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<RunPodEndpointObservation, ProviderClientError>> + Send + 'a,
        >,
    > {
        Box::pin(async move { RunPodClient::get_endpoint(self, api_key, endpoint_id).await })
    }

    fn find_endpoints_by_name<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        name: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<RunPodEndpointObservation>, ProviderClientError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move { RunPodClient::find_endpoints_by_name(self, api_key, name).await })
    }

    fn delete_endpoint<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        endpoint_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProviderClientError>> + Send + 'a>> {
        Box::pin(async move { RunPodClient::delete_endpoint(self, api_key, endpoint_id).await })
    }
}
