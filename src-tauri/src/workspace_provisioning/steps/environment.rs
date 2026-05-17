use crate::{
    domain::workspace::{
        provisioning_state::fail_workspace, Workspace, WorkspaceProvisioningPhase,
        WorkspaceProvisioningProgress, WorkspaceProvisioningStatus,
    },
    provider_resources::ProviderResourceGateway,
    provisioner_worker::{
        progress_from_worker_status, ProvisionerWorkerGateway, ProvisionerWorkerJobStatus,
        ProvisionerWorkerStartRequest,
    },
    secrets::{SecretStore, SecretStoreError},
    workspace_catalog::repository::WorkspaceCatalogRepository,
};

use super::super::{
    context::{SyncStepResult, WorkspaceProvisioningContext},
    failure,
    helpers::{result, WorkspaceProvisioningResult},
    WorkspaceProvisioningError,
};

pub(crate) async fn sync<S, P, W, R>(
    context: &WorkspaceProvisioningContext<'_, S, P, W, R>,
    workspace: &mut Workspace,
) -> SyncStepResult
where
    S: SecretStore,
    P: ProviderResourceGateway,
    W: WorkspaceCatalogRepository,
    R: ProvisionerWorkerGateway,
{
    if workspace.environment_prepared_at.is_some() {
        return Ok(None);
    }

    let Some(active_pod) = workspace.active_provisioning_pod_snapshot.clone() else {
        return Ok(None);
    };

    if active_pod.provider_resource_status
        != crate::domain::workspace::ProviderResourceStatus::Running
    {
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
                    return handle_worker_error(context, workspace.clone(), error.into()).await
                }
            }
        }
        Ok(status) if status.status == ProvisionerWorkerJobStatus::Succeeded => {
            workspace.environment_prepared_at = Some(now_rfc3339()?);
            let workspace = context.update_workspace(workspace).await?;
            return Ok(Some(result(workspace)));
        }
        Ok(status) => status,
        Err(error) => return handle_worker_error(context, workspace.clone(), error.into()).await,
    };
    Ok(Some(WorkspaceProvisioningResult {
        workspace: workspace.clone(),
        progress: progress_from_worker_status(&worker_status),
    }))
}

async fn handle_worker_error<S, P, W, R>(
    context: &WorkspaceProvisioningContext<'_, S, P, W, R>,
    mut workspace: Workspace,
    error: WorkspaceProvisioningError,
) -> SyncStepResult
where
    S: SecretStore,
    P: ProviderResourceGateway,
    W: WorkspaceCatalogRepository,
    R: ProvisionerWorkerGateway,
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
