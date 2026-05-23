use std::{future::Future, pin::Pin};

use crate::{
    domain::workspace::Workspace,
    provider::runpod::{RunPodClient, RunPodHttpClientInitError},
    secrets::{AsyncProviderKeyStore, AsyncProvisionerTokenStore},
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_resources::{
        WorkspaceResourceContext, WorkspaceResourceError, WorkspaceResourceOperationResult,
    },
};

use super::WorkspaceResourceProvider;

mod cleanup;
mod client;
mod context;
mod network_volume;
mod provisioning_pod;
mod serverless_endpoint;
#[cfg(test)]
mod test_support;

use client::RunPodWorkspaceResourceClient;
use context::RunPodWorkspaceResourceContext;

const RUNPOD_PROVISIONER_WORKER_HTTP_PORT: u16 = 8000;
const RUNPOD_ENDPOINT_COMFYUI_HTTP_PORT: u16 = 8188;
const GIB_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(super) struct RunPodWorkspaceResourceProvider {
    client: RunPodClient,
}

impl RunPodWorkspaceResourceProvider {
    pub(super) fn new(client: RunPodClient) -> Self {
        Self { client }
    }

    pub(super) fn try_new() -> Result<Self, RunPodHttpClientInitError> {
        Ok(Self::new(RunPodClient::try_new_default()?))
    }
}

impl<S, W> WorkspaceResourceProvider<S, W> for RunPodWorkspaceResourceProvider
where
    S: AsyncProviderKeyStore + AsyncProvisionerTokenStore,
    W: WorkspaceCatalogRepository,
{
    fn create_network_volume<'a>(
        &'a self,
        context: &'a WorkspaceResourceContext<'_, S, W>,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
        Box::pin(async move {
            create_network_volume_with_client(&self.client, context, workspace).await
        })
    }

    fn observe_network_volume<'a>(
        &'a self,
        context: &'a WorkspaceResourceContext<'_, S, W>,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
        Box::pin(async move {
            observe_network_volume_with_client(&self.client, context, workspace).await
        })
    }

    fn create_provisioning_pod<'a>(
        &'a self,
        context: &'a WorkspaceResourceContext<'_, S, W>,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
        Box::pin(async move {
            create_provisioning_pod_with_client(&self.client, context, workspace).await
        })
    }

    fn observe_provisioning_pod<'a>(
        &'a self,
        context: &'a WorkspaceResourceContext<'_, S, W>,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
        Box::pin(async move {
            observe_provisioning_pod_with_client(&self.client, context, workspace).await
        })
    }

    fn delete_provisioning_pod<'a>(
        &'a self,
        context: &'a WorkspaceResourceContext<'_, S, W>,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
        Box::pin(async move {
            delete_provisioning_pod_with_client(&self.client, context, workspace).await
        })
    }

    fn create_serverless_endpoint<'a>(
        &'a self,
        context: &'a WorkspaceResourceContext<'_, S, W>,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
        Box::pin(async move {
            create_serverless_endpoint_with_client(&self.client, context, workspace).await
        })
    }

    fn observe_serverless_endpoint<'a>(
        &'a self,
        context: &'a WorkspaceResourceContext<'_, S, W>,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
        Box::pin(async move {
            observe_serverless_endpoint_with_client(&self.client, context, workspace).await
        })
    }

    fn cleanup_known_resources<'a>(
        &'a self,
        context: &'a WorkspaceResourceContext<'_, S, W>,
        workspace: &'a Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceResourceError>> + Send + 'a>> {
        Box::pin(async move {
            cleanup::cleanup_known_resources_with_client(&self.client, context, workspace).await
        })
    }
}

async fn create_network_volume_with_client<S, W, C>(
    client: &C,
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &mut Workspace,
) -> WorkspaceResourceOperationResult
where
    S: AsyncProviderKeyStore,
    W: WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    let context = RunPodWorkspaceResourceContext::new(context, client);
    network_volume::create(&context, workspace).await
}

async fn observe_network_volume_with_client<S, W, C>(
    client: &C,
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &mut Workspace,
) -> WorkspaceResourceOperationResult
where
    S: AsyncProviderKeyStore,
    W: WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    let context = RunPodWorkspaceResourceContext::new(context, client);
    network_volume::observe(&context, workspace).await
}

async fn create_provisioning_pod_with_client<S, W, C>(
    client: &C,
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &mut Workspace,
) -> WorkspaceResourceOperationResult
where
    S: AsyncProviderKeyStore + AsyncProvisionerTokenStore,
    W: WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    let context = RunPodWorkspaceResourceContext::new(context, client);
    provisioning_pod::create(&context, workspace).await
}

async fn observe_provisioning_pod_with_client<S, W, C>(
    client: &C,
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &mut Workspace,
) -> WorkspaceResourceOperationResult
where
    S: AsyncProviderKeyStore,
    W: WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    let context = RunPodWorkspaceResourceContext::new(context, client);
    provisioning_pod::observe(&context, workspace).await
}

async fn delete_provisioning_pod_with_client<S, W, C>(
    client: &C,
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &mut Workspace,
) -> WorkspaceResourceOperationResult
where
    S: AsyncProviderKeyStore + AsyncProvisionerTokenStore,
    W: WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    let context = RunPodWorkspaceResourceContext::new(context, client);
    provisioning_pod::delete(&context, workspace).await
}

async fn create_serverless_endpoint_with_client<S, W, C>(
    client: &C,
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &mut Workspace,
) -> WorkspaceResourceOperationResult
where
    S: AsyncProviderKeyStore,
    W: WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    let context = RunPodWorkspaceResourceContext::new(context, client);
    serverless_endpoint::create(&context, workspace).await
}

async fn observe_serverless_endpoint_with_client<S, W, C>(
    client: &C,
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &mut Workspace,
) -> WorkspaceResourceOperationResult
where
    S: AsyncProviderKeyStore,
    W: WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    let context = RunPodWorkspaceResourceContext::new(context, client);
    serverless_endpoint::observe(&context, workspace).await
}
