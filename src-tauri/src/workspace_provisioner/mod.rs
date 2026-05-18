mod gateway;

pub(crate) use gateway::{
    progress_from_worker_status, ProvisionerWorkerError, ProvisionerWorkerGateway,
    ProvisionerWorkerHttpGateway, ProvisionerWorkerJobStatus, ProvisionerWorkerStartRequest,
};

#[cfg(test)]
pub(crate) use gateway::{ProvisionerWorkerPhase, ProvisionerWorkerStatus};

use crate::{
    domain::workspace::{
        provisioning_state::fail_workspace, ProviderResourceStatus, Workspace,
        WorkspaceProvisioningPhase, WorkspaceProvisioningProgress, WorkspaceProvisioningStatus,
    },
    secrets::{SecretStore, SecretStoreError},
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_provisioning::{
        failure,
        helpers::{result, WorkspaceProvisioningResult},
        WorkspaceProvisioningError,
    },
};

pub(crate) type WorkspaceProvisionerSyncResult =
    Result<Option<WorkspaceProvisioningResult>, WorkspaceProvisioningError>;

#[derive(Debug, Clone, Default)]
pub(crate) struct WorkspaceProvisionerService;

impl WorkspaceProvisionerService {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) async fn sync_environment<S, W, R>(
        &self,
        context: WorkspaceProvisionerContext<'_, S, W, R>,
        workspace: &mut Workspace,
    ) -> WorkspaceProvisionerSyncResult
    where
        S: SecretStore,
        W: WorkspaceCatalogRepository,
        R: ProvisionerWorkerGateway,
    {
        if workspace.environment_prepared_at.is_some() {
            return Ok(None);
        }

        let Some(active_pod) = workspace.active_provisioning_pod_snapshot.clone() else {
            return Ok(None);
        };

        if active_pod.provider_resource_status != ProviderResourceStatus::Running {
            return Ok(None);
        }

        let token = match context.secrets.read_provisioner_worker_token(&workspace.id) {
            Ok(Some(token)) => token,
            Ok(None) => {
                fail_workspace(
                    workspace,
                    failure::worker_token_missing(WorkspaceProvisioningPhase::PreparingEnvironment),
                );
                let workspace = context.update_workspace(workspace).await?;
                return Ok(Some(result(workspace)));
            }
            Err(SecretStoreError::InvalidStoredProvisionerWorkerToken) => {
                fail_workspace(
                    workspace,
                    failure::worker_token_invalid(WorkspaceProvisioningPhase::PreparingEnvironment),
                );
                let workspace = context.update_workspace(workspace).await?;
                return Ok(Some(result(workspace)));
            }
            Err(error) => return Err(WorkspaceProvisioningError::from(error)),
        };

        let worker_status = match context
            .workers
            .status(&active_pod.provisioner_status_url, &token)
            .await
        {
            Ok(status) if status.status == ProvisionerWorkerJobStatus::Idle => {
                match context
                    .workers
                    .start(
                        &active_pod.provisioner_status_url,
                        &token,
                        &ProvisionerWorkerStartRequest {
                            job_id: workspace.id.clone(),
                            workflow_preset: workspace
                                .placement_plan
                                .selected_workflow_preset()
                                .clone(),
                            resolved_runtime_image: workspace.resolved_runtime_image.clone(),
                        },
                    )
                    .await
                {
                    Ok(status) => status,
                    Err(error) => {
                        return handle_worker_error(&context, workspace.clone(), error.into()).await
                    }
                }
            }
            Ok(status) if status.status == ProvisionerWorkerJobStatus::Succeeded => {
                workspace.environment_prepared_at = Some(now_rfc3339()?);
                let workspace = context.update_workspace(workspace).await?;
                return Ok(Some(result(workspace)));
            }
            Ok(status) => status,
            Err(error) => {
                return handle_worker_error(&context, workspace.clone(), error.into()).await
            }
        };

        Ok(Some(WorkspaceProvisioningResult {
            workspace: workspace.clone(),
            progress: progress_from_worker_status(&worker_status),
        }))
    }
}

pub(crate) struct WorkspaceProvisionerContext<'a, S, W, R> {
    secrets: &'a S,
    workspace_catalog: &'a W,
    workers: &'a R,
}

impl<'a, S, W, R> WorkspaceProvisionerContext<'a, S, W, R> {
    pub(crate) fn new(secrets: &'a S, workspace_catalog: &'a W, workers: &'a R) -> Self {
        Self {
            secrets,
            workspace_catalog,
            workers,
        }
    }
}

impl<S, W, R> WorkspaceProvisionerContext<'_, S, W, R>
where
    W: WorkspaceCatalogRepository,
{
    async fn update_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, WorkspaceProvisioningError> {
        self.workspace_catalog
            .update_workspace(workspace)
            .await
            .map_err(catalog_error)
    }
}

async fn handle_worker_error<S, W, R>(
    context: &WorkspaceProvisionerContext<'_, S, W, R>,
    mut workspace: Workspace,
    error: WorkspaceProvisioningError,
) -> WorkspaceProvisionerSyncResult
where
    W: WorkspaceCatalogRepository,
{
    if error == WorkspaceProvisioningError::ProvisionerWorkerUnavailable {
        return Ok(Some(WorkspaceProvisioningResult {
            workspace,
            progress: worker_readiness_progress(),
        }));
    }

    if let Some(failure) =
        failure::worker_failure(WorkspaceProvisioningPhase::PreparingEnvironment, &error)
    {
        fail_workspace(&mut workspace, failure);
        let workspace = context.update_workspace(&workspace).await?;
        Ok(Some(result(workspace)))
    } else {
        Err(error)
    }
}

fn worker_readiness_progress() -> WorkspaceProvisioningProgress {
    WorkspaceProvisioningProgress {
        status: WorkspaceProvisioningStatus::Running,
        phase: WorkspaceProvisioningPhase::PreparingEnvironment,
        percent: None,
        failure: None,
    }
}

fn now_rfc3339() -> Result<String, WorkspaceProvisioningError> {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| WorkspaceProvisioningError::ProviderResponseInvalid)
}

fn catalog_error(
    _error: crate::workspace_setup::error::WorkspaceSetupError,
) -> WorkspaceProvisioningError {
    WorkspaceProvisioningError::WorkspaceCatalogUnavailable
}
