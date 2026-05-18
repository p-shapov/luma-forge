use crate::{
    domain::{
        provider_setup::GpuCloudProviderId,
        workspace::{provisioning_state::reset_after_resource_cleanup, Workspace},
    },
    secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
};

use super::{operations::runpod, WorkspaceResourceContext, WorkspaceResourceError};

pub(crate) type WorkspaceResourceSyncResult = Result<Option<Workspace>, WorkspaceResourceError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceResourceConfig {
    pub(crate) volume_mount_path: String,
}

#[derive(Debug, Clone)]
pub(crate) struct WorkspaceResourceService<S, W> {
    secrets: S,
    workspace_catalog: W,
}

impl<S, W> WorkspaceResourceService<S, W> {
    pub(crate) fn new(secrets: S, workspace_catalog: W) -> Self {
        Self {
            secrets,
            workspace_catalog,
        }
    }

    fn context(&self) -> WorkspaceResourceContext<'_, S, W> {
        WorkspaceResourceContext::new(&self.secrets, &self.workspace_catalog)
    }
}

impl<S, W> WorkspaceResourceService<S, W>
where
    S: SecretStore,
    W: WorkspaceCatalogRepository,
{
    pub(crate) async fn sync_network_volume(
        &self,
        workspace: &mut Workspace,
        config: &WorkspaceResourceConfig,
    ) -> WorkspaceResourceSyncResult {
        let context = self.context();
        match workspace.gpu_cloud_provider_id {
            GpuCloudProviderId::Runpod => {
                runpod::sync_network_volume(&context, workspace, config).await
            }
        }
    }

    pub(crate) async fn sync_provisioning_pod(
        &self,
        workspace: &mut Workspace,
        config: &WorkspaceResourceConfig,
    ) -> WorkspaceResourceSyncResult {
        let context = self.context();
        match workspace.gpu_cloud_provider_id {
            GpuCloudProviderId::Runpod => {
                runpod::sync_provisioning_pod(&context, workspace, config).await
            }
        }
    }

    pub(crate) async fn finish_provisioning_pod(
        &self,
        workspace: &mut Workspace,
    ) -> WorkspaceResourceSyncResult {
        let context = self.context();
        match workspace.gpu_cloud_provider_id {
            GpuCloudProviderId::Runpod => {
                runpod::finish_provisioning_pod(&context, workspace).await
            }
        }
    }

    pub(crate) async fn sync_serverless_endpoint(
        &self,
        workspace: &mut Workspace,
        config: &WorkspaceResourceConfig,
    ) -> WorkspaceResourceSyncResult {
        let context = self.context();
        match workspace.gpu_cloud_provider_id {
            GpuCloudProviderId::Runpod => {
                runpod::sync_serverless_endpoint(&context, workspace, config).await
            }
        }
    }

    pub(crate) async fn cleanup_known_resources(
        &self,
        workspace: &mut Workspace,
    ) -> Result<Workspace, WorkspaceResourceError> {
        let context = self.context();
        match workspace.gpu_cloud_provider_id {
            GpuCloudProviderId::Runpod => {
                runpod::cleanup_known_resources(&context, workspace).await?
            }
        }
        reset_after_resource_cleanup(workspace);
        context.update_workspace(workspace).await
    }
}
