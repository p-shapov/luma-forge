use std::{future::Future, pin::Pin};

use crate::{
    domain::workspace::Workspace,
    provider::runpod::RunPodClient,
    secrets::AsyncSecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_resources::{
        WorkspaceResourceConfig, WorkspaceResourceContext, WorkspaceResourceError,
        WorkspaceResourceSyncResult,
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

const RUNPOD_VOLUME_MOUNT_PATH: &str = "/workspace";
const RUNPOD_PROVISIONER_WORKER_HTTP_PORT: u16 = 8000;
const RUNPOD_ENDPOINT_COMFYUI_HTTP_PORT: u16 = 8188;
const GIB_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Default)]
pub(super) struct RunPodWorkspaceResourceProvider;

impl<S, W> WorkspaceResourceProvider<S, W> for RunPodWorkspaceResourceProvider
where
    S: AsyncSecretStore,
    W: WorkspaceCatalogRepository,
{
    fn sync_network_volume<'a>(
        &'a self,
        context: &'a WorkspaceResourceContext<'_, S, W>,
        workspace: &'a mut Workspace,
        config: &'a WorkspaceResourceConfig,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceSyncResult> + Send + 'a>> {
        Box::pin(async move {
            crate::workspace_resources::providers::runpod::sync_network_volume(
                context, workspace, config,
            )
            .await
        })
    }

    fn sync_provisioning_pod<'a>(
        &'a self,
        context: &'a WorkspaceResourceContext<'_, S, W>,
        workspace: &'a mut Workspace,
        config: &'a WorkspaceResourceConfig,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceSyncResult> + Send + 'a>> {
        Box::pin(async move {
            crate::workspace_resources::providers::runpod::sync_provisioning_pod(
                context, workspace, config,
            )
            .await
        })
    }

    fn finish_provisioning_pod<'a>(
        &'a self,
        context: &'a WorkspaceResourceContext<'_, S, W>,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceSyncResult> + Send + 'a>> {
        Box::pin(async move {
            crate::workspace_resources::providers::runpod::finish_provisioning_pod(
                context, workspace,
            )
            .await
        })
    }

    fn sync_serverless_endpoint<'a>(
        &'a self,
        context: &'a WorkspaceResourceContext<'_, S, W>,
        workspace: &'a mut Workspace,
        config: &'a WorkspaceResourceConfig,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceSyncResult> + Send + 'a>> {
        Box::pin(async move {
            crate::workspace_resources::providers::runpod::sync_serverless_endpoint(
                context, workspace, config,
            )
            .await
        })
    }

    fn cleanup_known_resources<'a>(
        &'a self,
        context: &'a WorkspaceResourceContext<'_, S, W>,
        workspace: &'a Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceResourceError>> + Send + 'a>> {
        Box::pin(async move {
            crate::workspace_resources::providers::runpod::cleanup_known_resources(
                context, workspace,
            )
            .await
        })
    }
}

pub(crate) async fn sync_network_volume<S, W>(
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &mut Workspace,
    config: &WorkspaceResourceConfig,
) -> WorkspaceResourceSyncResult
where
    S: AsyncSecretStore,
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
    S: AsyncSecretStore,
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
    S: AsyncSecretStore,
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
    S: AsyncSecretStore,
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
    S: AsyncSecretStore,
    W: WorkspaceCatalogRepository,
{
    cleanup::cleanup_known_resources(context, workspace).await
}

async fn sync_network_volume_with_client<S, W, C>(
    client: &C,
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &mut Workspace,
    config: &WorkspaceResourceConfig,
) -> WorkspaceResourceSyncResult
where
    S: AsyncSecretStore,
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
    S: AsyncSecretStore,
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
    S: AsyncSecretStore,
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
    S: AsyncSecretStore,
    W: WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    let context = RunPodWorkspaceResourceContext::new(context, client);
    serverless_endpoint::sync(&context, workspace, config).await
}
