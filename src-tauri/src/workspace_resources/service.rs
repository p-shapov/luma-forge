use crate::{
    domain::{
        provider_setup::GpuCloudProviderId,
        workspace::{provisioning_state::reset_after_resource_cleanup, Workspace},
    },
    secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
};

use super::{operations::runpod, WorkspaceResourceError};

pub(crate) type WorkspaceResourceSyncResult = Result<Option<Workspace>, WorkspaceResourceError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceResourceConfig {
    pub(crate) volume_mount_path: String,
}

#[derive(Debug, Clone)]
pub(crate) struct WorkspaceResourceService<S, W> {
    pub(crate) secrets: S,
    pub(crate) workspace_catalog: W,
}

impl<S, W> WorkspaceResourceService<S, W> {
    pub(crate) fn new(secrets: S, workspace_catalog: W) -> Self {
        Self {
            secrets,
            workspace_catalog,
        }
    }
}

impl<S, W> WorkspaceResourceService<S, W>
where
    W: WorkspaceCatalogRepository,
{
    pub(crate) async fn update_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, WorkspaceResourceError> {
        self.workspace_catalog
            .update_workspace(workspace)
            .await
            .map_err(WorkspaceResourceError::from)
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
        match workspace.gpu_cloud_provider_id {
            GpuCloudProviderId::Runpod => {
                runpod::sync_network_volume(self, workspace, config).await
            }
        }
    }

    pub(crate) async fn sync_provisioning_pod(
        &self,
        workspace: &mut Workspace,
        config: &WorkspaceResourceConfig,
    ) -> WorkspaceResourceSyncResult {
        match workspace.gpu_cloud_provider_id {
            GpuCloudProviderId::Runpod => {
                runpod::sync_provisioning_pod(self, workspace, config).await
            }
        }
    }

    pub(crate) async fn finish_provisioning_pod(
        &self,
        workspace: &mut Workspace,
    ) -> WorkspaceResourceSyncResult {
        match workspace.gpu_cloud_provider_id {
            GpuCloudProviderId::Runpod => runpod::finish_provisioning_pod(self, workspace).await,
        }
    }

    pub(crate) async fn sync_serverless_endpoint(
        &self,
        workspace: &mut Workspace,
        config: &WorkspaceResourceConfig,
    ) -> WorkspaceResourceSyncResult {
        match workspace.gpu_cloud_provider_id {
            GpuCloudProviderId::Runpod => {
                runpod::sync_serverless_endpoint(self, workspace, config).await
            }
        }
    }

    pub(crate) async fn cleanup_known_resources(
        &self,
        workspace: &mut Workspace,
    ) -> Result<Workspace, WorkspaceResourceError> {
        match workspace.gpu_cloud_provider_id {
            GpuCloudProviderId::Runpod => runpod::cleanup_known_resources(self, workspace).await?,
        }
        reset_after_resource_cleanup(workspace);
        self.update_workspace(workspace).await
    }
}
