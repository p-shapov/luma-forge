use std::{collections::HashMap, future::Future, pin::Pin};

use crate::{
    domain::{
        provider_setup::{GpuCloudProviderId, ProviderApiKey},
        workspace::{ProviderProvisioningSnapshot, Workspace},
    },
    provider::{
        runpod::{
            RunPodClient, RunPodCreateEndpointRequest, RunPodCreateNetworkVolumeRequest,
            RunPodCreatePodRequest, RunPodCreateTemplateRequest, RunPodEndpointObservation,
            RunPodNetworkVolumeObservation, RunPodPodObservation, RunPodTemplateObservation,
        },
        ProviderClientError,
    },
    secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_resources::{
        provider_resource_name, CreateEndpointTemplateInput, CreateNetworkVolumeInput,
        CreateProvisioningPodInput, CreateServerlessEndpointInput, DiscoverEndpointTemplatesInput,
        DiscoverNetworkVolumesInput, DiscoverProvisioningPodsInput,
        DiscoverServerlessEndpointsInput, EndpointTemplateObservation, NetworkVolumeObservation,
        ObserveProvisioningPodInput, ProvisioningPodObservation, ServerlessEndpointObservation,
        WorkspaceResourceError,
    },
};

use crate::workspace_resources::{
    WorkspaceResourceConfig, WorkspaceResourceContext, WorkspaceResourceSyncResult,
};

mod network_volume;
mod provisioning_pod;
mod serverless_endpoint;

const RUNPOD_VOLUME_MOUNT_PATH: &str = "/workspace";
const RUNPOD_PROVISIONER_WORKER_HTTP_PORT: u16 = 8000;
const RUNPOD_ENDPOINT_COMFYUI_HTTP_PORT: u16 = 8188;
const GIB_BYTES: u64 = 1024 * 1024 * 1024;

pub(crate) async fn sync_network_volume<S, W>(
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &mut Workspace,
    config: &WorkspaceResourceConfig,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: WorkspaceCatalogRepository,
{
    let client = RunPodClient::default();
    sync_network_volume_with_client(&client, context, workspace, config).await
}

pub(crate) async fn sync_provisioning_pod<S, W>(
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &mut Workspace,
    config: &WorkspaceResourceConfig,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: WorkspaceCatalogRepository,
{
    let client = RunPodClient::default();
    sync_provisioning_pod_with_client(&client, context, workspace, config).await
}

pub(crate) async fn finish_provisioning_pod<S, W>(
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &mut Workspace,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: WorkspaceCatalogRepository,
{
    let client = RunPodClient::default();
    finish_provisioning_pod_with_client(&client, context, workspace).await
}

pub(crate) async fn sync_serverless_endpoint<S, W>(
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &mut Workspace,
    config: &WorkspaceResourceConfig,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: WorkspaceCatalogRepository,
{
    let client = RunPodClient::default();
    sync_serverless_endpoint_with_client(&client, context, workspace, config).await
}

pub(crate) async fn cleanup_known_resources<S, W>(
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &Workspace,
) -> Result<(), WorkspaceResourceError>
where
    S: SecretStore,
    W: WorkspaceCatalogRepository,
{
    let client = RunPodClient::default();
    cleanup_known_resources_with_client(&client, context, workspace).await
}

async fn sync_network_volume_with_client<S, W, C>(
    client: &C,
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &mut Workspace,
    config: &WorkspaceResourceConfig,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    let context = RunPodWorkspaceResourceContext::new(context, client);
    network_volume::sync(&context, workspace, config).await
}

async fn sync_provisioning_pod_with_client<S, W, C>(
    client: &C,
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &mut Workspace,
    config: &WorkspaceResourceConfig,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    let context = RunPodWorkspaceResourceContext::new(context, client);
    provisioning_pod::sync(&context, workspace, config).await
}

async fn finish_provisioning_pod_with_client<S, W, C>(
    client: &C,
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &mut Workspace,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    let context = RunPodWorkspaceResourceContext::new(context, client);
    provisioning_pod::finish(&context, workspace).await
}

async fn sync_serverless_endpoint_with_client<S, W, C>(
    client: &C,
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &mut Workspace,
    config: &WorkspaceResourceConfig,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    let context = RunPodWorkspaceResourceContext::new(context, client);
    serverless_endpoint::sync(&context, workspace, config).await
}

async fn cleanup_known_resources_with_client<S, W, C>(
    client: &C,
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &Workspace,
) -> Result<(), WorkspaceResourceError>
where
    S: SecretStore,
    W: WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    let context = RunPodWorkspaceResourceContext::new(context, client);
    let mut first_error = None;

    if let Some(endpoint) = &workspace.serverless_endpoint_snapshot {
        remember_first_error(
            &mut first_error,
            tolerate_missing(
                context
                    .delete_serverless_endpoint(
                        workspace.gpu_cloud_provider_id,
                        &endpoint.provider_resource_id,
                    )
                    .await,
            ),
        );
    }

    if let Some(template_id) = runpod_template_id(workspace) {
        remember_first_error(
            &mut first_error,
            tolerate_missing(
                context
                    .delete_endpoint_template(workspace.gpu_cloud_provider_id, &template_id)
                    .await,
            ),
        );
    }

    if let Some(active_pod) = &workspace.active_provisioning_pod_snapshot {
        remember_first_error(
            &mut first_error,
            tolerate_missing(
                context
                    .delete_provisioning_pod(
                        workspace.gpu_cloud_provider_id,
                        &active_pod.provider_resource_id,
                    )
                    .await,
            ),
        );
    }

    if let Some(volume) = &workspace.persistent_storage_volume_snapshot {
        remember_first_error(
            &mut first_error,
            tolerate_missing(
                context
                    .delete_network_volume(
                        workspace.gpu_cloud_provider_id,
                        &volume.provider_resource_id,
                    )
                    .await,
            ),
        );
    }

    remember_first_error(
        &mut first_error,
        context
            .secrets
            .delete_provisioner_worker_token(&workspace.id)
            .map_err(WorkspaceResourceError::from),
    );

    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn runpod_template_id(workspace: &Workspace) -> Option<String> {
    match &workspace.provider_provisioning_snapshot {
        Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot: Some(snapshot),
        }) => Some(snapshot.template_id.clone()),
        _ => None,
    }
}

fn tolerate_missing(
    result: Result<(), WorkspaceResourceError>,
) -> Result<(), WorkspaceResourceError> {
    match result {
        Ok(()) | Err(WorkspaceResourceError::ProviderResourceNotFound) => Ok(()),
        Err(error) => Err(error),
    }
}

fn remember_first_error(
    first_error: &mut Option<WorkspaceResourceError>,
    result: Result<(), WorkspaceResourceError>,
) {
    if first_error.is_none() {
        if let Err(error) = result {
            *first_error = Some(error);
        }
    }
}

struct RunPodWorkspaceResourceContext<'a, S, W, C> {
    base: &'a WorkspaceResourceContext<'a, S, W>,
    client: &'a C,
    pub(crate) secrets: &'a S,
}

impl<'a, S, W, C> RunPodWorkspaceResourceContext<'a, S, W, C> {
    fn new(base: &'a WorkspaceResourceContext<'a, S, W>, client: &'a C) -> Self {
        Self {
            base,
            client,
            secrets: base.secrets,
        }
    }
}

impl<S, W, C> RunPodWorkspaceResourceContext<'_, S, W, C>
where
    W: WorkspaceCatalogRepository,
{
    async fn update_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, WorkspaceResourceError> {
        self.base.update_workspace(workspace).await
    }
}

trait RunPodWorkspaceResourceClient: Send + Sync {
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

    fn get_template<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        template_id: &'a str,
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

    fn get_template<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
        template_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<RunPodTemplateObservation, ProviderClientError>> + Send + 'a,
        >,
    > {
        Box::pin(async move { RunPodClient::get_template(self, api_key, template_id).await })
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

impl<S, W, C> RunPodWorkspaceResourceContext<'_, S, W, C>
where
    S: SecretStore,
    C: RunPodWorkspaceResourceClient,
{
    async fn create_network_volume(
        &self,
        input: CreateNetworkVolumeInput,
    ) -> Result<NetworkVolumeObservation, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
        self.client
            .create_network_volume(
                &api_key,
                &RunPodCreateNetworkVolumeRequest {
                    name: provider_resource_name(&input.workspace_id, "volume"),
                    data_center_id: input.datacenter_id,
                    size: bytes_to_gib(input.size_bytes),
                },
            )
            .await
            .map(runpod_network_volume_observation)
            .map_err(WorkspaceResourceError::from)
    }

    async fn get_network_volume(
        &self,
        provider_id: GpuCloudProviderId,
        volume_id: &str,
    ) -> Result<NetworkVolumeObservation, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&provider_id)?;
        self.client
            .get_network_volume(&api_key, volume_id)
            .await
            .map(runpod_network_volume_observation)
            .map_err(WorkspaceResourceError::from)
    }

    async fn discover_network_volumes(
        &self,
        input: DiscoverNetworkVolumesInput,
    ) -> Result<Vec<NetworkVolumeObservation>, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
        self.client
            .find_network_volumes_by_name(
                &api_key,
                &provider_resource_name(&input.workspace_id, "volume"),
            )
            .await
            .map(|observations| {
                observations
                    .into_iter()
                    .map(runpod_network_volume_observation)
                    .collect()
            })
            .map_err(WorkspaceResourceError::from)
    }

    async fn delete_network_volume(
        &self,
        provider_id: GpuCloudProviderId,
        volume_id: &str,
    ) -> Result<(), WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&provider_id)?;
        self.client
            .delete_network_volume(&api_key, volume_id)
            .await
            .map_err(WorkspaceResourceError::from)
    }

    async fn create_provisioning_pod(
        &self,
        input: CreateProvisioningPodInput,
    ) -> Result<ProvisioningPodObservation, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
        self.client
            .create_pod(
                &api_key,
                &RunPodCreatePodRequest {
                    name: provider_resource_name(&input.workspace_id, "provisioner"),
                    image_name: input.provisioner_worker_image_ref,
                    gpu_type_ids: vec![input.selected_gpu_id],
                    data_center_ids: vec![input.datacenter_id],
                    network_volume_id: input.network_volume_id,
                    volume_mount_path: input.mount_path,
                    env: HashMap::from([(
                        "LUMA_FORGE_PROVISIONER_BEARER_TOKEN".to_string(),
                        input.bearer_token.expose_secret().to_string(),
                    )]),
                    ports: vec![format!("{RUNPOD_PROVISIONER_WORKER_HTTP_PORT}/http")],
                },
            )
            .await
            .map(runpod_pod_observation)
            .map_err(WorkspaceResourceError::from)
    }

    async fn discover_provisioning_pods(
        &self,
        input: DiscoverProvisioningPodsInput,
    ) -> Result<Vec<ProvisioningPodObservation>, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
        self.client
            .find_pods_by_name(
                &api_key,
                &provider_resource_name(&input.workspace_id, "provisioner"),
            )
            .await
            .map(|observations| {
                observations
                    .into_iter()
                    .map(runpod_pod_observation)
                    .collect()
            })
            .map_err(WorkspaceResourceError::from)
    }

    async fn get_provisioning_pod(
        &self,
        input: ObserveProvisioningPodInput,
    ) -> Result<ProvisioningPodObservation, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
        self.client
            .get_pod(&api_key, &input.provider_resource_id)
            .await
            .map(runpod_pod_observation)
            .map_err(WorkspaceResourceError::from)
    }

    async fn delete_provisioning_pod(
        &self,
        provider_id: GpuCloudProviderId,
        pod_id: &str,
    ) -> Result<(), WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&provider_id)?;
        self.client
            .delete_pod(&api_key, pod_id)
            .await
            .map_err(WorkspaceResourceError::from)
    }

    async fn create_endpoint_template(
        &self,
        input: CreateEndpointTemplateInput,
    ) -> Result<EndpointTemplateObservation, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
        self.client
            .create_template(
                &api_key,
                &RunPodCreateTemplateRequest {
                    name: provider_resource_name(&input.workspace_id, "endpoint-template"),
                    image_name: input.endpoint_worker_image_ref,
                    container_disk_in_gb: 10,
                    env: HashMap::new(),
                    is_public: false,
                    is_serverless: true,
                    ports: vec![format!("{RUNPOD_ENDPOINT_COMFYUI_HTTP_PORT}/http")],
                    readme: String::new(),
                    volume_mount_path: input.mount_path,
                },
            )
            .await
            .map(runpod_template_observation)
            .map_err(WorkspaceResourceError::from)
    }

    async fn discover_endpoint_templates(
        &self,
        input: DiscoverEndpointTemplatesInput,
    ) -> Result<Vec<EndpointTemplateObservation>, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
        self.client
            .find_templates_by_name(
                &api_key,
                &provider_resource_name(&input.workspace_id, "endpoint-template"),
            )
            .await
            .map(|observations| {
                observations
                    .into_iter()
                    .map(runpod_template_observation)
                    .collect()
            })
            .map_err(WorkspaceResourceError::from)
    }

    async fn get_endpoint_template(
        &self,
        provider_id: GpuCloudProviderId,
        template_id: &str,
    ) -> Result<EndpointTemplateObservation, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&provider_id)?;
        self.client
            .get_template(&api_key, template_id)
            .await
            .map(runpod_template_observation)
            .map_err(WorkspaceResourceError::from)
    }

    async fn delete_endpoint_template(
        &self,
        provider_id: GpuCloudProviderId,
        template_id: &str,
    ) -> Result<(), WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&provider_id)?;
        self.client
            .delete_template(&api_key, template_id)
            .await
            .map_err(WorkspaceResourceError::from)
    }

    async fn create_serverless_endpoint(
        &self,
        input: CreateServerlessEndpointInput,
    ) -> Result<ServerlessEndpointObservation, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
        self.client
            .create_endpoint(
                &api_key,
                &RunPodCreateEndpointRequest {
                    name: provider_resource_name(&input.workspace_id, "endpoint"),
                    template_id: input.template_id,
                    gpu_type_ids: vec![input.selected_gpu_id],
                    network_volume_id: input.network_volume_id,
                    data_center_ids: vec![input.datacenter_id],
                    workers_min: 0,
                    workers_max: 1,
                    scaler_type: "REQUEST_COUNT".to_string(),
                    scaler_value: 1,
                    idle_timeout: input.endpoint_keep_alive_seconds,
                },
            )
            .await
            .map(runpod_endpoint_observation)
            .map_err(WorkspaceResourceError::from)
    }

    async fn discover_serverless_endpoints(
        &self,
        input: DiscoverServerlessEndpointsInput,
    ) -> Result<Vec<ServerlessEndpointObservation>, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
        self.client
            .find_endpoints_by_name(
                &api_key,
                &provider_resource_name(&input.workspace_id, "endpoint"),
            )
            .await
            .map(|observations| {
                observations
                    .into_iter()
                    .map(runpod_endpoint_observation)
                    .collect()
            })
            .map_err(WorkspaceResourceError::from)
    }

    async fn get_serverless_endpoint(
        &self,
        provider_id: GpuCloudProviderId,
        endpoint_id: &str,
    ) -> Result<ServerlessEndpointObservation, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&provider_id)?;
        self.client
            .get_endpoint(&api_key, endpoint_id)
            .await
            .map(runpod_endpoint_observation)
            .map_err(WorkspaceResourceError::from)
    }

    async fn delete_serverless_endpoint(
        &self,
        provider_id: GpuCloudProviderId,
        endpoint_id: &str,
    ) -> Result<(), WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&provider_id)?;
        self.client
            .delete_endpoint(&api_key, endpoint_id)
            .await
            .map_err(WorkspaceResourceError::from)
    }

    fn provisioning_api_key(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> Result<ProviderApiKey, WorkspaceResourceError> {
        self.secrets
            .read_api_key(provider_id)
            .map_err(WorkspaceResourceError::from)?
            .ok_or(WorkspaceResourceError::ProviderSetupIncomplete)
    }
}

fn runpod_network_volume_observation(
    observation: RunPodNetworkVolumeObservation,
) -> NetworkVolumeObservation {
    NetworkVolumeObservation {
        provider_resource_id: observation.id,
        provider_resource_status: observation.status,
        mount_path: RUNPOD_VOLUME_MOUNT_PATH.to_string(),
    }
}

fn runpod_pod_observation(observation: RunPodPodObservation) -> ProvisioningPodObservation {
    ProvisioningPodObservation {
        provider_resource_id: observation.id,
        provider_resource_status: observation.status,
        provisioner_status_url: observation.provisioner_status_url,
    }
}

fn runpod_template_observation(
    observation: RunPodTemplateObservation,
) -> EndpointTemplateObservation {
    EndpointTemplateObservation {
        template_id: observation.id,
        endpoint_worker_image_ref: observation.image_name,
        mount_path: observation.volume_mount_path,
        provider_resource_status: observation.status,
    }
}

fn runpod_endpoint_observation(
    observation: RunPodEndpointObservation,
) -> ServerlessEndpointObservation {
    ServerlessEndpointObservation {
        provider_resource_id: observation.id,
        provider_resource_status: observation.status,
        endpoint_invoke_url: observation.endpoint_invoke_url,
    }
}

fn bytes_to_gib(bytes: u64) -> u64 {
    bytes.div_ceil(GIB_BYTES)
}

#[cfg(test)]
pub(super) mod test_support {
    use super::*;
    use crate::{
        domain::{
            placement::PlacementPlan,
            runtime::ResolvedRuntimeImageSnapshot,
            workflow::{RuntimeContractReference, WorkflowExecutionType, WorkflowPreset},
            workspace::{
                PersistentStorageVolumeSnapshot, ProviderResourceStatus, ProvisioningPodSnapshot,
                ServerlessEndpointSnapshot, Workspace, WorkspaceCatalog, WorkspaceLifecycleState,
            },
        },
        secrets::{ProvisionerWorkerBearerToken, SecretStoreError},
        workspace_setup::error::WorkspaceSetupError,
    };
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    #[allow(dead_code)]
    #[derive(Debug, Clone)]
    pub(super) enum RunPodCall {
        CreateNetworkVolume(RunPodCreateNetworkVolumeRequest),
        GetNetworkVolume(String),
        DiscoverNetworkVolumes(String),
        DeleteNetworkVolume(String),
        CreatePod(RunPodCreatePodRequest),
        GetPod(String),
        DiscoverPods(String),
        DeletePod(String),
        CreateTemplate(RunPodCreateTemplateRequest),
        GetTemplate(String),
        DiscoverTemplates(String),
        DeleteTemplate(String),
        CreateEndpoint(RunPodCreateEndpointRequest),
        GetEndpoint(String),
        DiscoverEndpoints(String),
        DeleteEndpoint(String),
    }

    #[derive(Debug, Default)]
    pub(super) struct FakeRunPodClient {
        calls: Mutex<Vec<RunPodCall>>,
        create_network_volume_results:
            Mutex<VecDeque<Result<RunPodNetworkVolumeObservation, ProviderClientError>>>,
        get_network_volume_results:
            Mutex<VecDeque<Result<RunPodNetworkVolumeObservation, ProviderClientError>>>,
        discover_network_volume_results:
            Mutex<VecDeque<Result<Vec<RunPodNetworkVolumeObservation>, ProviderClientError>>>,
        delete_network_volume_results: Mutex<VecDeque<Result<(), ProviderClientError>>>,
        create_pod_results: Mutex<VecDeque<Result<RunPodPodObservation, ProviderClientError>>>,
        get_pod_results: Mutex<VecDeque<Result<RunPodPodObservation, ProviderClientError>>>,
        discover_pod_results:
            Mutex<VecDeque<Result<Vec<RunPodPodObservation>, ProviderClientError>>>,
        delete_pod_results: Mutex<VecDeque<Result<(), ProviderClientError>>>,
        create_template_results:
            Mutex<VecDeque<Result<RunPodTemplateObservation, ProviderClientError>>>,
        get_template_results:
            Mutex<VecDeque<Result<RunPodTemplateObservation, ProviderClientError>>>,
        discover_template_results:
            Mutex<VecDeque<Result<Vec<RunPodTemplateObservation>, ProviderClientError>>>,
        delete_template_results: Mutex<VecDeque<Result<(), ProviderClientError>>>,
        create_endpoint_results:
            Mutex<VecDeque<Result<RunPodEndpointObservation, ProviderClientError>>>,
        get_endpoint_results:
            Mutex<VecDeque<Result<RunPodEndpointObservation, ProviderClientError>>>,
        discover_endpoint_results:
            Mutex<VecDeque<Result<Vec<RunPodEndpointObservation>, ProviderClientError>>>,
        delete_endpoint_results: Mutex<VecDeque<Result<(), ProviderClientError>>>,
    }

    impl FakeRunPodClient {
        pub(super) fn calls(&self) -> Vec<RunPodCall> {
            self.calls.lock().expect("fake runpod calls").clone()
        }

        pub(super) fn push_create_network_volume(
            &self,
            result: Result<RunPodNetworkVolumeObservation, ProviderClientError>,
        ) {
            self.create_network_volume_results
                .lock()
                .expect("fake create volume results")
                .push_back(result);
        }

        pub(super) fn push_get_network_volume(
            &self,
            result: Result<RunPodNetworkVolumeObservation, ProviderClientError>,
        ) {
            self.get_network_volume_results
                .lock()
                .expect("fake get volume results")
                .push_back(result);
        }

        pub(super) fn push_discover_network_volumes(
            &self,
            result: Result<Vec<RunPodNetworkVolumeObservation>, ProviderClientError>,
        ) {
            self.discover_network_volume_results
                .lock()
                .expect("fake discover volume results")
                .push_back(result);
        }

        pub(super) fn push_delete_network_volume(&self, result: Result<(), ProviderClientError>) {
            self.delete_network_volume_results
                .lock()
                .expect("fake delete volume results")
                .push_back(result);
        }

        pub(super) fn push_create_pod(
            &self,
            result: Result<RunPodPodObservation, ProviderClientError>,
        ) {
            self.create_pod_results
                .lock()
                .expect("fake create pod results")
                .push_back(result);
        }

        pub(super) fn push_get_pod(
            &self,
            result: Result<RunPodPodObservation, ProviderClientError>,
        ) {
            self.get_pod_results
                .lock()
                .expect("fake get pod results")
                .push_back(result);
        }

        pub(super) fn push_discover_pods(
            &self,
            result: Result<Vec<RunPodPodObservation>, ProviderClientError>,
        ) {
            self.discover_pod_results
                .lock()
                .expect("fake discover pod results")
                .push_back(result);
        }

        pub(super) fn push_delete_pod(&self, result: Result<(), ProviderClientError>) {
            self.delete_pod_results
                .lock()
                .expect("fake delete pod results")
                .push_back(result);
        }

        pub(super) fn push_create_template(
            &self,
            result: Result<RunPodTemplateObservation, ProviderClientError>,
        ) {
            self.create_template_results
                .lock()
                .expect("fake create template results")
                .push_back(result);
        }

        pub(super) fn push_get_template(
            &self,
            result: Result<RunPodTemplateObservation, ProviderClientError>,
        ) {
            self.get_template_results
                .lock()
                .expect("fake get template results")
                .push_back(result);
        }

        pub(super) fn push_discover_templates(
            &self,
            result: Result<Vec<RunPodTemplateObservation>, ProviderClientError>,
        ) {
            self.discover_template_results
                .lock()
                .expect("fake discover template results")
                .push_back(result);
        }

        pub(super) fn push_delete_template(&self, result: Result<(), ProviderClientError>) {
            self.delete_template_results
                .lock()
                .expect("fake delete template results")
                .push_back(result);
        }

        pub(super) fn push_create_endpoint(
            &self,
            result: Result<RunPodEndpointObservation, ProviderClientError>,
        ) {
            self.create_endpoint_results
                .lock()
                .expect("fake create endpoint results")
                .push_back(result);
        }

        pub(super) fn push_get_endpoint(
            &self,
            result: Result<RunPodEndpointObservation, ProviderClientError>,
        ) {
            self.get_endpoint_results
                .lock()
                .expect("fake get endpoint results")
                .push_back(result);
        }

        pub(super) fn push_discover_endpoints(
            &self,
            result: Result<Vec<RunPodEndpointObservation>, ProviderClientError>,
        ) {
            self.discover_endpoint_results
                .lock()
                .expect("fake discover endpoint results")
                .push_back(result);
        }

        pub(super) fn push_delete_endpoint(&self, result: Result<(), ProviderClientError>) {
            self.delete_endpoint_results
                .lock()
                .expect("fake delete endpoint results")
                .push_back(result);
        }

        fn record(&self, call: RunPodCall) {
            self.calls.lock().expect("fake runpod calls").push(call);
        }

        fn next<T>(
            queue: &Mutex<VecDeque<Result<T, ProviderClientError>>>,
            label: &str,
        ) -> Result<T, ProviderClientError> {
            queue
                .lock()
                .expect(label)
                .pop_front()
                .unwrap_or_else(|| panic!("missing fake result for {label}"))
        }
    }

    impl RunPodWorkspaceResourceClient for FakeRunPodClient {
        fn create_network_volume<'a>(
            &'a self,
            _api_key: &'a ProviderApiKey,
            request: &'a RunPodCreateNetworkVolumeRequest,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<RunPodNetworkVolumeObservation, ProviderClientError>>
                    + Send
                    + 'a,
            >,
        > {
            self.record(RunPodCall::CreateNetworkVolume(request.clone()));
            Box::pin(async move {
                Self::next(
                    &self.create_network_volume_results,
                    "fake create volume results",
                )
            })
        }

        fn get_network_volume<'a>(
            &'a self,
            _api_key: &'a ProviderApiKey,
            volume_id: &'a str,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<RunPodNetworkVolumeObservation, ProviderClientError>>
                    + Send
                    + 'a,
            >,
        > {
            self.record(RunPodCall::GetNetworkVolume(volume_id.to_string()));
            Box::pin(async move {
                Self::next(&self.get_network_volume_results, "fake get volume results")
            })
        }

        fn find_network_volumes_by_name<'a>(
            &'a self,
            _api_key: &'a ProviderApiKey,
            name: &'a str,
        ) -> Pin<
            Box<
                dyn Future<
                        Output = Result<Vec<RunPodNetworkVolumeObservation>, ProviderClientError>,
                    > + Send
                    + 'a,
            >,
        > {
            self.record(RunPodCall::DiscoverNetworkVolumes(name.to_string()));
            Box::pin(async move {
                Self::next(
                    &self.discover_network_volume_results,
                    "fake discover volume results",
                )
            })
        }

        fn delete_network_volume<'a>(
            &'a self,
            _api_key: &'a ProviderApiKey,
            volume_id: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<(), ProviderClientError>> + Send + 'a>> {
            self.record(RunPodCall::DeleteNetworkVolume(volume_id.to_string()));
            Box::pin(async move {
                Self::next(
                    &self.delete_network_volume_results,
                    "fake delete volume results",
                )
            })
        }

        fn create_pod<'a>(
            &'a self,
            _api_key: &'a ProviderApiKey,
            request: &'a RunPodCreatePodRequest,
        ) -> Pin<
            Box<dyn Future<Output = Result<RunPodPodObservation, ProviderClientError>> + Send + 'a>,
        > {
            self.record(RunPodCall::CreatePod(request.clone()));
            Box::pin(async move { Self::next(&self.create_pod_results, "fake create pod results") })
        }

        fn get_pod<'a>(
            &'a self,
            _api_key: &'a ProviderApiKey,
            pod_id: &'a str,
        ) -> Pin<
            Box<dyn Future<Output = Result<RunPodPodObservation, ProviderClientError>> + Send + 'a>,
        > {
            self.record(RunPodCall::GetPod(pod_id.to_string()));
            Box::pin(async move { Self::next(&self.get_pod_results, "fake get pod results") })
        }

        fn find_pods_by_name<'a>(
            &'a self,
            _api_key: &'a ProviderApiKey,
            name: &'a str,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<Vec<RunPodPodObservation>, ProviderClientError>>
                    + Send
                    + 'a,
            >,
        > {
            self.record(RunPodCall::DiscoverPods(name.to_string()));
            Box::pin(
                async move { Self::next(&self.discover_pod_results, "fake discover pod results") },
            )
        }

        fn delete_pod<'a>(
            &'a self,
            _api_key: &'a ProviderApiKey,
            pod_id: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<(), ProviderClientError>> + Send + 'a>> {
            self.record(RunPodCall::DeletePod(pod_id.to_string()));
            Box::pin(async move { Self::next(&self.delete_pod_results, "fake delete pod results") })
        }

        fn create_template<'a>(
            &'a self,
            _api_key: &'a ProviderApiKey,
            request: &'a RunPodCreateTemplateRequest,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<RunPodTemplateObservation, ProviderClientError>>
                    + Send
                    + 'a,
            >,
        > {
            self.record(RunPodCall::CreateTemplate(request.clone()));
            Box::pin(async move {
                Self::next(
                    &self.create_template_results,
                    "fake create template results",
                )
            })
        }

        fn get_template<'a>(
            &'a self,
            _api_key: &'a ProviderApiKey,
            template_id: &'a str,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<RunPodTemplateObservation, ProviderClientError>>
                    + Send
                    + 'a,
            >,
        > {
            self.record(RunPodCall::GetTemplate(template_id.to_string()));
            Box::pin(
                async move { Self::next(&self.get_template_results, "fake get template results") },
            )
        }

        fn find_templates_by_name<'a>(
            &'a self,
            _api_key: &'a ProviderApiKey,
            name: &'a str,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<Vec<RunPodTemplateObservation>, ProviderClientError>>
                    + Send
                    + 'a,
            >,
        > {
            self.record(RunPodCall::DiscoverTemplates(name.to_string()));
            Box::pin(async move {
                Self::next(
                    &self.discover_template_results,
                    "fake discover template results",
                )
            })
        }

        fn delete_template<'a>(
            &'a self,
            _api_key: &'a ProviderApiKey,
            template_id: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<(), ProviderClientError>> + Send + 'a>> {
            self.record(RunPodCall::DeleteTemplate(template_id.to_string()));
            Box::pin(async move {
                Self::next(
                    &self.delete_template_results,
                    "fake delete template results",
                )
            })
        }

        fn create_endpoint<'a>(
            &'a self,
            _api_key: &'a ProviderApiKey,
            request: &'a RunPodCreateEndpointRequest,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<RunPodEndpointObservation, ProviderClientError>>
                    + Send
                    + 'a,
            >,
        > {
            self.record(RunPodCall::CreateEndpoint(request.clone()));
            Box::pin(async move {
                Self::next(
                    &self.create_endpoint_results,
                    "fake create endpoint results",
                )
            })
        }

        fn get_endpoint<'a>(
            &'a self,
            _api_key: &'a ProviderApiKey,
            endpoint_id: &'a str,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<RunPodEndpointObservation, ProviderClientError>>
                    + Send
                    + 'a,
            >,
        > {
            self.record(RunPodCall::GetEndpoint(endpoint_id.to_string()));
            Box::pin(
                async move { Self::next(&self.get_endpoint_results, "fake get endpoint results") },
            )
        }

        fn find_endpoints_by_name<'a>(
            &'a self,
            _api_key: &'a ProviderApiKey,
            name: &'a str,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<Vec<RunPodEndpointObservation>, ProviderClientError>>
                    + Send
                    + 'a,
            >,
        > {
            self.record(RunPodCall::DiscoverEndpoints(name.to_string()));
            Box::pin(async move {
                Self::next(
                    &self.discover_endpoint_results,
                    "fake discover endpoint results",
                )
            })
        }

        fn delete_endpoint<'a>(
            &'a self,
            _api_key: &'a ProviderApiKey,
            endpoint_id: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<(), ProviderClientError>> + Send + 'a>> {
            self.record(RunPodCall::DeleteEndpoint(endpoint_id.to_string()));
            Box::pin(async move {
                Self::next(
                    &self.delete_endpoint_results,
                    "fake delete endpoint results",
                )
            })
        }
    }

    #[derive(Debug, Clone)]
    pub(super) struct FakeSecretStore {
        state: Arc<Mutex<FakeSecretStoreState>>,
    }

    #[derive(Debug)]
    struct FakeSecretStoreState {
        api_key: Option<ProviderApiKey>,
        write_tokens: Vec<(String, String)>,
        delete_token_calls: Vec<String>,
        write_token_error: Option<SecretStoreError>,
        delete_token_error: Option<SecretStoreError>,
    }

    impl Default for FakeSecretStore {
        fn default() -> Self {
            Self {
                state: Arc::new(Mutex::new(FakeSecretStoreState {
                    api_key: Some(
                        ProviderApiKey::new("rp_test_key".to_string())
                            .expect("test api key should be valid"),
                    ),
                    write_tokens: Vec::new(),
                    delete_token_calls: Vec::new(),
                    write_token_error: None,
                    delete_token_error: None,
                })),
            }
        }
    }

    impl FakeSecretStore {
        pub(super) fn write_tokens(&self) -> Vec<(String, String)> {
            self.state
                .lock()
                .expect("fake secret store")
                .write_tokens
                .clone()
        }

        pub(super) fn delete_token_calls(&self) -> Vec<String> {
            self.state
                .lock()
                .expect("fake secret store")
                .delete_token_calls
                .clone()
        }

        pub(super) fn fail_delete_token(&self, error: SecretStoreError) {
            self.state
                .lock()
                .expect("fake secret store")
                .delete_token_error = Some(error);
        }
    }

    impl SecretStore for FakeSecretStore {
        fn has_api_key_entry(
            &self,
            _provider_id: &GpuCloudProviderId,
        ) -> Result<bool, SecretStoreError> {
            Ok(self
                .state
                .lock()
                .expect("fake secret store")
                .api_key
                .is_some())
        }

        fn read_api_key(
            &self,
            _provider_id: &GpuCloudProviderId,
        ) -> Result<Option<ProviderApiKey>, SecretStoreError> {
            Ok(self
                .state
                .lock()
                .expect("fake secret store")
                .api_key
                .clone())
        }

        fn replace_api_key(
            &self,
            _provider_id: &GpuCloudProviderId,
            api_key: &ProviderApiKey,
        ) -> Result<(), SecretStoreError> {
            self.state.lock().expect("fake secret store").api_key = Some(api_key.clone());
            Ok(())
        }

        fn delete_api_key(
            &self,
            _provider_id: &GpuCloudProviderId,
        ) -> Result<(), SecretStoreError> {
            self.state.lock().expect("fake secret store").api_key = None;
            Ok(())
        }

        fn write_provisioner_worker_token(
            &self,
            workspace_id: &str,
            token: &ProvisionerWorkerBearerToken,
        ) -> Result<(), SecretStoreError> {
            let mut state = self.state.lock().expect("fake secret store");
            state
                .write_tokens
                .push((workspace_id.to_string(), token.expose_secret().to_string()));
            match state.write_token_error.clone() {
                Some(error) => Err(error),
                None => Ok(()),
            }
        }

        fn read_provisioner_worker_token(
            &self,
            _workspace_id: &str,
        ) -> Result<Option<ProvisionerWorkerBearerToken>, SecretStoreError> {
            Ok(None)
        }

        fn delete_provisioner_worker_token(
            &self,
            workspace_id: &str,
        ) -> Result<(), SecretStoreError> {
            let mut state = self.state.lock().expect("fake secret store");
            state.delete_token_calls.push(workspace_id.to_string());
            match state.delete_token_error.clone() {
                Some(error) => Err(error),
                None => Ok(()),
            }
        }
    }

    #[derive(Debug, Default)]
    pub(super) struct FakeWorkspaceCatalog {
        updates: Mutex<Vec<Workspace>>,
    }

    impl FakeWorkspaceCatalog {
        pub(super) fn updates(&self) -> Vec<Workspace> {
            self.updates.lock().expect("fake catalog updates").clone()
        }
    }

    impl WorkspaceCatalogRepository for FakeWorkspaceCatalog {
        fn list_workspaces<'a>(
            &'a self,
        ) -> Pin<Box<dyn Future<Output = Result<WorkspaceCatalog, WorkspaceSetupError>> + Send + 'a>>
        {
            Box::pin(async { Err(WorkspaceSetupError::WorkspaceCatalogUnavailable) })
        }

        fn find_workspace_by_id<'a>(
            &'a self,
            _id: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<Option<Workspace>, WorkspaceSetupError>> + Send + 'a>>
        {
            Box::pin(async { Err(WorkspaceSetupError::WorkspaceCatalogUnavailable) })
        }

        fn insert_workspace<'a>(
            &'a self,
            _workspace: &'a Workspace,
        ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>>
        {
            Box::pin(async { Err(WorkspaceSetupError::WorkspaceCatalogUnavailable) })
        }

        fn update_workspace<'a>(
            &'a self,
            workspace: &'a Workspace,
        ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>>
        {
            Box::pin(async move {
                self.updates
                    .lock()
                    .expect("fake catalog updates")
                    .push(workspace.clone());
                Ok(workspace.clone())
            })
        }
    }

    pub(super) fn context<'a>(
        secrets: &'a FakeSecretStore,
        catalog: &'a FakeWorkspaceCatalog,
    ) -> WorkspaceResourceContext<'a, FakeSecretStore, FakeWorkspaceCatalog> {
        WorkspaceResourceContext::new(secrets, catalog)
    }

    pub(super) fn config() -> WorkspaceResourceConfig {
        WorkspaceResourceConfig {
            volume_mount_path: "/workspace".to_string(),
        }
    }

    pub(super) fn workspace() -> Workspace {
        let preset = WorkflowPreset {
            id: "preset-1".to_string(),
            version: "1.0.0".to_string(),
            name: "Preset".to_string(),
            workflow_execution_type: WorkflowExecutionType::T2i,
            required_base_volume_size_bytes: 1,
            runtime_contract: RuntimeContractReference {
                id: "runtime".to_string(),
                version: "1.0.0".to_string(),
            },
            required_model_assets: Vec::new(),
            required_custom_nodes: Vec::new(),
        };
        let placement_plan = PlacementPlan::Runpod {
            selected_datacenter_id: "dc-1".to_string(),
            selected_gpu_id: "gpu-1".to_string(),
            persistent_storage_volume_size_bytes: 1,
            endpoint_keep_alive_seconds: 5,
            selected_workflow_preset: preset,
        };
        let runtime = ResolvedRuntimeImageSnapshot {
            contract_id: "runtime".to_string(),
            contract_version: "1.0.0".to_string(),
            provisioner_image_ref: "provisioner:latest".to_string(),
            endpoint_image_ref: "endpoint:latest".to_string(),
        };
        let mut workspace = Workspace::new_draft(
            GpuCloudProviderId::Runpod,
            "workspace-1".to_string(),
            "Workspace".to_string(),
            placement_plan,
            runtime,
        )
        .expect("test workspace should be valid");
        workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
        workspace
    }

    pub(super) fn volume_snapshot(
        status: ProviderResourceStatus,
    ) -> PersistentStorageVolumeSnapshot {
        PersistentStorageVolumeSnapshot {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            provider_resource_id: "volume-1".to_string(),
            provider_resource_status: status,
            mount_path: "/workspace".to_string(),
        }
    }

    pub(super) fn pod_snapshot(status: ProviderResourceStatus) -> ProvisioningPodSnapshot {
        ProvisioningPodSnapshot {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            provider_resource_id: "pod-1".to_string(),
            provider_resource_status: status,
            provisioner_status_url: "https://pod/status".to_string(),
        }
    }

    pub(super) fn endpoint_snapshot(status: ProviderResourceStatus) -> ServerlessEndpointSnapshot {
        ServerlessEndpointSnapshot {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            provider_resource_id: "endpoint-1".to_string(),
            provider_resource_status: status,
            endpoint_invoke_url: "https://endpoint/run".to_string(),
        }
    }

    pub(super) fn runpod_volume(
        id: &str,
        status: ProviderResourceStatus,
    ) -> RunPodNetworkVolumeObservation {
        RunPodNetworkVolumeObservation {
            id: id.to_string(),
            status,
        }
    }

    pub(super) fn runpod_pod(
        id: &str,
        status: ProviderResourceStatus,
        provisioner_status_url: Option<&str>,
    ) -> RunPodPodObservation {
        RunPodPodObservation {
            id: id.to_string(),
            status,
            provisioner_status_url: provisioner_status_url.map(str::to_string),
        }
    }

    pub(super) fn runpod_template(
        id: &str,
        status: ProviderResourceStatus,
        image_name: &str,
        mount_path: &str,
    ) -> RunPodTemplateObservation {
        RunPodTemplateObservation {
            id: id.to_string(),
            image_name: image_name.to_string(),
            volume_mount_path: mount_path.to_string(),
            status,
        }
    }

    pub(super) fn runpod_endpoint(
        id: &str,
        status: ProviderResourceStatus,
    ) -> RunPodEndpointObservation {
        RunPodEndpointObservation {
            id: id.to_string(),
            status,
            endpoint_invoke_url: format!("https://endpoint/{id}/run"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{cleanup_known_resources_with_client, test_support::*};
    use crate::{
        domain::workspace::{
            ProviderProvisioningSnapshot, ProviderResourceStatus, RunPodEndpointTemplateSnapshot,
        },
        provider::ProviderClientError,
        secrets::SecretStoreError,
        workspace_resources::WorkspaceResourceError,
    };

    fn workspace_with_all_resources() -> crate::domain::workspace::Workspace {
        let mut workspace = workspace();
        workspace.persistent_storage_volume_snapshot =
            Some(volume_snapshot(ProviderResourceStatus::Ready));
        workspace.active_provisioning_pod_snapshot =
            Some(pod_snapshot(ProviderResourceStatus::Running));
        workspace.serverless_endpoint_snapshot =
            Some(endpoint_snapshot(ProviderResourceStatus::Ready));
        workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot: Some(RunPodEndpointTemplateSnapshot {
                template_id: "template-1".to_string(),
                provider_resource_status: ProviderResourceStatus::Ready,
                endpoint_worker_image_ref: "endpoint:latest".to_string(),
                mount_path: "/workspace".to_string(),
            }),
        });
        workspace
    }

    #[tokio::test]
    async fn cleanup_deletes_known_resources_in_dependency_order() {
        let client = FakeRunPodClient::default();
        let secrets = FakeSecretStore::default();
        let catalog = FakeWorkspaceCatalog::default();
        let context = context(&secrets, &catalog);
        let workspace = workspace_with_all_resources();
        client.push_delete_endpoint(Ok(()));
        client.push_delete_template(Ok(()));
        client.push_delete_pod(Ok(()));
        client.push_delete_network_volume(Ok(()));

        cleanup_known_resources_with_client(&client, &context, &workspace)
            .await
            .expect("cleanup should succeed");

        let calls = client.calls();
        assert!(matches!(calls[0], RunPodCall::DeleteEndpoint(_)));
        assert!(matches!(calls[1], RunPodCall::DeleteTemplate(_)));
        assert!(matches!(calls[2], RunPodCall::DeletePod(_)));
        assert!(matches!(calls[3], RunPodCall::DeleteNetworkVolume(_)));
        assert_eq!(
            secrets.delete_token_calls(),
            vec!["workspace-1".to_string()]
        );
    }

    #[tokio::test]
    async fn cleanup_tolerates_provider_not_found() {
        let client = FakeRunPodClient::default();
        let secrets = FakeSecretStore::default();
        let catalog = FakeWorkspaceCatalog::default();
        let context = context(&secrets, &catalog);
        let workspace = workspace_with_all_resources();
        client.push_delete_endpoint(Err(ProviderClientError::NotFound));
        client.push_delete_template(Err(ProviderClientError::NotFound));
        client.push_delete_pod(Err(ProviderClientError::NotFound));
        client.push_delete_network_volume(Err(ProviderClientError::NotFound));

        cleanup_known_resources_with_client(&client, &context, &workspace)
            .await
            .expect("not found resources should be tolerated");

        assert_eq!(client.calls().len(), 4);
        assert_eq!(
            secrets.delete_token_calls(),
            vec!["workspace-1".to_string()]
        );
    }

    #[tokio::test]
    async fn cleanup_continues_after_first_real_error_and_returns_it() {
        let client = FakeRunPodClient::default();
        let secrets = FakeSecretStore::default();
        let catalog = FakeWorkspaceCatalog::default();
        let context = context(&secrets, &catalog);
        let workspace = workspace_with_all_resources();
        client.push_delete_endpoint(Err(ProviderClientError::ApiUnavailable));
        client.push_delete_template(Ok(()));
        client.push_delete_pod(Ok(()));
        client.push_delete_network_volume(Ok(()));

        let error = cleanup_known_resources_with_client(&client, &context, &workspace)
            .await
            .expect_err("first real provider error should be returned");

        assert_eq!(error, WorkspaceResourceError::ProviderApiUnavailable);
        assert_eq!(client.calls().len(), 4);
        assert_eq!(
            secrets.delete_token_calls(),
            vec!["workspace-1".to_string()]
        );
    }

    #[tokio::test]
    async fn cleanup_returns_token_delete_error_after_provider_cleanup() {
        let client = FakeRunPodClient::default();
        let secrets = FakeSecretStore::default();
        secrets.fail_delete_token(SecretStoreError::SecureKeyringUnavailable);
        let catalog = FakeWorkspaceCatalog::default();
        let context = context(&secrets, &catalog);
        let workspace = workspace_with_all_resources();
        client.push_delete_endpoint(Ok(()));
        client.push_delete_template(Ok(()));
        client.push_delete_pod(Ok(()));
        client.push_delete_network_volume(Ok(()));

        let error = cleanup_known_resources_with_client(&client, &context, &workspace)
            .await
            .expect_err("token delete error should be returned");

        assert_eq!(error, WorkspaceResourceError::SecureKeyringUnavailable);
        assert_eq!(client.calls().len(), 4);
    }
}
