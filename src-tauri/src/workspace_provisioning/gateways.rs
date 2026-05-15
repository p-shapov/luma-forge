use std::{future::Future, pin::Pin};

use crate::{
    domain::provider_setup::GpuCloudProviderId,
    provisioner_worker::{
        ProvisionerWorkerError, ProvisionerWorkerStartRequest, ProvisionerWorkerStatus,
    },
    secrets::ProvisionerWorkerBearerToken,
};

use super::{
    contracts::{
        CreateEndpointTemplateInput, CreateNetworkVolumeInput, CreateProvisioningPodInput,
        CreateServerlessEndpointInput, EndpointTemplateObservation, NetworkVolumeObservation,
        ProvisioningPodObservation, ServerlessEndpointObservation,
    },
    WorkspaceProvisioningError,
};

pub trait ProviderProvisioningGateway: Send + Sync {
    fn create_network_volume<'a>(
        &'a self,
        input: CreateNetworkVolumeInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<NetworkVolumeObservation, WorkspaceProvisioningError>>
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
            dyn Future<Output = Result<NetworkVolumeObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    >;

    fn delete_network_volume<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        volume_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceProvisioningError>> + Send + 'a>>;

    fn create_provisioning_pod<'a>(
        &'a self,
        input: CreateProvisioningPodInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisioningPodObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    >;

    fn get_provisioning_pod<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        pod_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisioningPodObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    >;

    fn delete_provisioning_pod<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        pod_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceProvisioningError>> + Send + 'a>>;

    fn create_endpoint_template<'a>(
        &'a self,
        input: CreateEndpointTemplateInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<EndpointTemplateObservation, WorkspaceProvisioningError>>
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
            dyn Future<Output = Result<EndpointTemplateObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    >;

    fn delete_endpoint_template<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        template_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceProvisioningError>> + Send + 'a>>;

    fn create_serverless_endpoint<'a>(
        &'a self,
        input: CreateServerlessEndpointInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ServerlessEndpointObservation, WorkspaceProvisioningError>>
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
            dyn Future<Output = Result<ServerlessEndpointObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    >;

    fn delete_serverless_endpoint<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        endpoint_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceProvisioningError>> + Send + 'a>>;
}

pub trait ProvisionerWorkerGateway: Send + Sync {
    fn start<'a>(
        &'a self,
        provisioner_status_url: &'a str,
        token: &'a ProvisionerWorkerBearerToken,
        request: &'a ProvisionerWorkerStartRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisionerWorkerStatus, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    >;

    fn status<'a>(
        &'a self,
        provisioner_status_url: &'a str,
        token: &'a ProvisionerWorkerBearerToken,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisionerWorkerStatus, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    >;

    fn cancel<'a>(
        &'a self,
        provisioner_status_url: &'a str,
        token: &'a ProvisionerWorkerBearerToken,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisionerWorkerStatus, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    >;
}

impl ProvisionerWorkerGateway for crate::provisioner_worker::ProvisionerWorkerHttpGateway {
    fn start<'a>(
        &'a self,
        provisioner_status_url: &'a str,
        token: &'a ProvisionerWorkerBearerToken,
        request: &'a ProvisionerWorkerStartRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisionerWorkerStatus, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.start(provisioner_status_url, token, request)
                .await
                .map_err(worker_error)
        })
    }

    fn status<'a>(
        &'a self,
        provisioner_status_url: &'a str,
        token: &'a ProvisionerWorkerBearerToken,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisionerWorkerStatus, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.status(provisioner_status_url, token)
                .await
                .map_err(worker_error)
        })
    }

    fn cancel<'a>(
        &'a self,
        provisioner_status_url: &'a str,
        token: &'a ProvisionerWorkerBearerToken,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisionerWorkerStatus, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.cancel(provisioner_status_url, token)
                .await
                .map_err(worker_error)
        })
    }
}

fn worker_error(error: ProvisionerWorkerError) -> WorkspaceProvisioningError {
    match error {
        ProvisionerWorkerError::Unauthorized => {
            WorkspaceProvisioningError::ProvisionerWorkerUnauthorized
        }
        ProvisionerWorkerError::Conflict => WorkspaceProvisioningError::ProvisionerWorkerConflict,
        ProvisionerWorkerError::Unreachable => {
            WorkspaceProvisioningError::ProvisionerWorkerUnavailable
        }
        ProvisionerWorkerError::InvalidPayload => {
            WorkspaceProvisioningError::ProvisionerWorkerResponseInvalid
        }
        ProvisionerWorkerError::TerminalFailure { diagnostic } => {
            WorkspaceProvisioningError::ProvisionerWorkerFailed { diagnostic }
        }
    }
}
