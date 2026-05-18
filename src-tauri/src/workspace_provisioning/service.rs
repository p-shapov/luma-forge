use crate::{
    domain::workspace::{provisioning_state::fail_workspace, WorkspaceLifecycleState},
    secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_provisioner::ProvisionerWorkerGateway,
    workspace_provisioner::WorkspaceProvisionerService,
    workspace_resources::WorkspaceResourceService,
};

use super::{
    context::WorkspaceProvisioningContext,
    coordinator::WorkspaceProvisioningCoordinator,
    failure,
    helpers::{result, WorkspaceProvisioningResult},
    steps, WorkspaceProvisioningError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceProvisioningConfig {
    pub volume_mount_path: String,
}

pub struct WorkspaceProvisioningService<S, W, R> {
    secrets: S,
    resources: WorkspaceResourceService<S, W>,
    workspace_catalog: W,
    workers: R,
    workspace_provisioner: WorkspaceProvisionerService,
    coordinator: WorkspaceProvisioningCoordinator,
    config: WorkspaceProvisioningConfig,
}

impl<S, W, R> WorkspaceProvisioningService<S, W, R> {
    pub fn new(
        secrets: S,
        resources: WorkspaceResourceService<S, W>,
        workspace_catalog: W,
        workers: R,
        coordinator: WorkspaceProvisioningCoordinator,
        config: WorkspaceProvisioningConfig,
    ) -> Self {
        Self {
            secrets,
            resources,
            workspace_catalog,
            workers,
            workspace_provisioner: WorkspaceProvisionerService::new(),
            coordinator,
            config,
        }
    }

    fn context(&self) -> WorkspaceProvisioningContext<'_, S, W, R> {
        WorkspaceProvisioningContext::new(
            &self.secrets,
            &self.resources,
            &self.workspace_catalog,
            &self.workers,
            &self.workspace_provisioner,
            &self.config,
        )
    }
}

impl<S, W, R> WorkspaceProvisioningService<S, W, R>
where
    S: SecretStore,
    W: WorkspaceCatalogRepository,
    R: ProvisionerWorkerGateway,
{
    pub async fn initiate(
        &self,
        workspace_id: &str,
    ) -> Result<WorkspaceProvisioningResult, WorkspaceProvisioningError> {
        let context = self.context();
        let mut workspace = context.workspace(workspace_id).await?;
        if workspace.lifecycle_state != WorkspaceLifecycleState::Draft {
            return Err(WorkspaceProvisioningError::InvalidWorkspaceLifecycle);
        }
        self.secrets
            .read_api_key(&workspace.gpu_cloud_provider_id)
            .map_err(WorkspaceProvisioningError::from)?
            .ok_or(WorkspaceProvisioningError::ProviderSetupIncomplete)?;

        workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
        workspace.last_provisioning_failure = None;
        let workspace = context.update_workspace(&workspace).await?;
        Ok(result(workspace))
    }

    pub async fn sync(
        &self,
        workspace_id: &str,
    ) -> Result<WorkspaceProvisioningResult, WorkspaceProvisioningError> {
        let Some(_guard) = self.coordinator.try_enter(workspace_id) else {
            let context = self.context();
            return Ok(result(context.workspace(workspace_id).await?));
        };

        let context = self.context();
        let mut workspace = context.workspace(workspace_id).await?;
        if workspace.lifecycle_state != WorkspaceLifecycleState::Provisioning {
            return Ok(result(workspace));
        }

        if let Some(result) = steps::sync(&context, &mut workspace).await? {
            return Ok(result);
        }

        Ok(result(workspace))
    }

    pub async fn cancel(
        &self,
        workspace_id: &str,
    ) -> Result<WorkspaceProvisioningResult, WorkspaceProvisioningError> {
        let Some(_guard) = self.coordinator.try_enter(workspace_id) else {
            return Err(WorkspaceProvisioningError::ProviderOperationConflict);
        };

        let context = self.context();
        let mut workspace = context.workspace(workspace_id).await?;
        if workspace.lifecycle_state != WorkspaceLifecycleState::Provisioning {
            return Err(WorkspaceProvisioningError::InvalidWorkspaceLifecycle);
        }

        match self.resources.cleanup_known_resources(&mut workspace).await {
            Ok(updated_workspace) => {
                workspace = updated_workspace;
            }
            Err(_) => {
                fail_workspace(&mut workspace, failure::cancellation_cleanup_failed());
                workspace = context.update_workspace(&workspace).await?;
            }
        }

        Ok(result(workspace))
    }
}
