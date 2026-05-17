use crate::{
    domain::workspace::{
        provisioning_state::{fail_workspace, reset_after_resource_cleanup},
        WorkspaceLifecycleState,
    },
    provider_resources::ProviderResourceGateway,
    provisioner_worker::ProvisionerWorkerGateway,
    secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
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

pub struct WorkspaceProvisioningService<S, P, W, R> {
    secrets: S,
    providers: P,
    workspace_catalog: W,
    workers: R,
    coordinator: WorkspaceProvisioningCoordinator,
    config: WorkspaceProvisioningConfig,
}

impl<S, P, W, R> WorkspaceProvisioningService<S, P, W, R> {
    pub fn new(
        secrets: S,
        providers: P,
        workspace_catalog: W,
        workers: R,
        coordinator: WorkspaceProvisioningCoordinator,
        config: WorkspaceProvisioningConfig,
    ) -> Self {
        Self {
            secrets,
            providers,
            workspace_catalog,
            workers,
            coordinator,
            config,
        }
    }

    fn context(&self) -> WorkspaceProvisioningContext<'_, S, P, W, R> {
        WorkspaceProvisioningContext::new(
            &self.secrets,
            &self.providers,
            &self.workspace_catalog,
            &self.workers,
            &self.config,
        )
    }
}

impl<S, P, W, R> WorkspaceProvisioningService<S, P, W, R>
where
    S: SecretStore,
    P: ProviderResourceGateway,
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

        if let Some(result) = steps::network_volume::sync(&context, &mut workspace).await? {
            return Ok(result);
        }
        if let Some(result) = steps::provisioning_pod::sync(&context, &mut workspace).await? {
            return Ok(result);
        }
        if let Some(result) = steps::environment::sync(&context, &mut workspace).await? {
            return Ok(result);
        }
        if let Some(result) = steps::provisioning_pod::finish(&context, &mut workspace).await? {
            return Ok(result);
        }
        if let Some(result) = steps::endpoint_template::sync(&context, &mut workspace).await? {
            return Ok(result);
        }
        if let Some(result) = steps::serverless_endpoint::sync(&context, &mut workspace).await? {
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

        match crate::workspace_resource_cleanup::cleanup_known_resources(
            &self.secrets,
            &self.providers,
            &workspace,
        )
        .await
        {
            Ok(()) => {
                reset_after_resource_cleanup(&mut workspace);
            }
            Err(_) => {
                fail_workspace(&mut workspace, failure::cancellation_cleanup_failed());
            }
        }

        let workspace = context.update_workspace(&workspace).await?;
        Ok(result(workspace))
    }
}
