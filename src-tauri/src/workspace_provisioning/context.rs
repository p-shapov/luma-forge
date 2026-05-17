use crate::{
    domain::workspace::{
        provisioning_state::{
            fail_workspace, is_terminal_provider_resource_status, runpod_template_snapshot,
        },
        Workspace, WorkspaceProvisioningPhase,
    },
    provider_resources::ProviderResourceGateway,
    provisioner_worker::ProvisionerWorkerGateway,
    secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
};

use super::{
    failure,
    helpers::{result, WorkspaceProvisioningResult},
    WorkspaceProvisioningConfig, WorkspaceProvisioningError,
};

pub(crate) type SyncStepResult =
    Result<Option<WorkspaceProvisioningResult>, WorkspaceProvisioningError>;

pub(crate) struct WorkspaceProvisioningContext<'a, S, P, W, R> {
    pub(crate) secrets: &'a S,
    pub(crate) providers: &'a P,
    pub(crate) workspace_catalog: &'a W,
    pub(crate) workers: &'a R,
    pub(crate) config: &'a WorkspaceProvisioningConfig,
}

impl<'a, S, P, W, R> WorkspaceProvisioningContext<'a, S, P, W, R> {
    pub(crate) fn new(
        secrets: &'a S,
        providers: &'a P,
        workspace_catalog: &'a W,
        workers: &'a R,
        config: &'a WorkspaceProvisioningConfig,
    ) -> Self {
        Self {
            secrets,
            providers,
            workspace_catalog,
            workers,
            config,
        }
    }
}

impl<S, P, W, R> WorkspaceProvisioningContext<'_, S, P, W, R>
where
    S: SecretStore,
    P: ProviderResourceGateway,
    W: WorkspaceCatalogRepository,
    R: ProvisionerWorkerGateway,
{
    pub(crate) async fn workspace(
        &self,
        workspace_id: &str,
    ) -> Result<Workspace, WorkspaceProvisioningError> {
        self.workspace_catalog
            .find_workspace_by_id(workspace_id)
            .await
            .map_err(catalog_error)?
            .ok_or(WorkspaceProvisioningError::WorkspaceNotFound)
    }

    pub(crate) async fn update_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, WorkspaceProvisioningError> {
        self.workspace_catalog
            .update_workspace(workspace)
            .await
            .map_err(catalog_error)
    }

    pub(crate) async fn fail_for_indeterminate_provider_operation(
        &self,
        workspace: &mut Workspace,
        phase: WorkspaceProvisioningPhase,
    ) -> SyncStepResult {
        fail_workspace(workspace, failure::indeterminate_provider_operation(phase));
        let workspace = self.update_workspace(workspace).await?;
        Ok(Some(result(workspace)))
    }

    pub(crate) async fn fail_for_missing_provider_resource(
        &self,
        workspace: &mut Workspace,
        phase: WorkspaceProvisioningPhase,
    ) -> SyncStepResult {
        fail_workspace(workspace, failure::missing_provider_resource(phase));
        let workspace = self.update_workspace(workspace).await?;
        Ok(Some(result(workspace)))
    }

    pub(crate) async fn fail_for_orphaned_provider_resources(
        &self,
        workspace: &mut Workspace,
        phase: WorkspaceProvisioningPhase,
        provider_resource_ids: Vec<String>,
    ) -> SyncStepResult {
        fail_workspace(
            workspace,
            failure::orphaned_provider_resources(phase, provider_resource_ids),
        );
        let workspace = self.update_workspace(workspace).await?;
        Ok(Some(result(workspace)))
    }

    pub(crate) fn fail_if_volume_status_is_terminal(&self, workspace: &mut Workspace) {
        if let Some(status) = workspace
            .persistent_storage_volume_snapshot
            .as_ref()
            .map(|snapshot| snapshot.provider_resource_status.clone())
            .filter(is_terminal_provider_resource_status)
        {
            let failure = failure::provider_resource_failure(
                WorkspaceProvisioningPhase::CreatingVolume,
                &status,
            );
            fail_workspace(workspace, failure);
        }
    }

    pub(crate) fn fail_if_template_status_is_terminal(&self, workspace: &mut Workspace) {
        if let Some(status) = runpod_template_snapshot(workspace)
            .map(|snapshot| snapshot.provider_resource_status)
            .filter(is_terminal_provider_resource_status)
        {
            let failure = failure::provider_resource_failure(
                WorkspaceProvisioningPhase::CreatingEndpointTemplate,
                &status,
            );
            fail_workspace(workspace, failure);
        }
    }

    pub(crate) fn fail_if_endpoint_status_is_terminal(&self, workspace: &mut Workspace) {
        if let Some(status) = workspace
            .serverless_endpoint_snapshot
            .as_ref()
            .map(|snapshot| snapshot.provider_resource_status.clone())
            .filter(is_terminal_provider_resource_status)
        {
            let failure = failure::provider_resource_failure(
                WorkspaceProvisioningPhase::CreatingEndpoint,
                &status,
            );
            fail_workspace(workspace, failure);
        }
    }
}

fn catalog_error(
    _error: crate::workspace_setup::error::WorkspaceSetupError,
) -> WorkspaceProvisioningError {
    WorkspaceProvisioningError::WorkspaceCatalogUnavailable
}
