use super::{
    ProviderProvisioningSnapshot, ProviderResourceStatus, RunPodEndpointTemplateSnapshot,
    Workspace, WorkspaceLifecycleState, WorkspaceProvisioningFailure,
    WorkspaceProvisioningFailureCode, WorkspaceProvisioningFailureSource,
    WorkspaceProvisioningPhase, WorkspaceProvisioningProgress, WorkspaceProvisioningRecoveryAction,
    WorkspaceProvisioningStatus,
};

pub(crate) fn fail_workspace(workspace: &mut Workspace, failure: WorkspaceProvisioningFailure) {
    workspace.lifecycle_state = WorkspaceLifecycleState::Failed;
    workspace.last_provisioning_failure = Some(failure);
}

pub(crate) fn reset_after_resource_cleanup(workspace: &mut Workspace) {
    workspace.lifecycle_state = WorkspaceLifecycleState::Draft;
    workspace.persistent_storage_volume_snapshot = None;
    workspace.active_provisioning_pod_snapshot = None;
    workspace.serverless_endpoint_snapshot = None;
    workspace.last_provisioning_pod_snapshot = None;
    workspace.provider_provisioning_snapshot = None;
    workspace.environment_prepared_at = None;
    workspace.last_provisioning_failure = None;
}

pub(crate) fn runpod_template_snapshot(
    workspace: &Workspace,
) -> Option<RunPodEndpointTemplateSnapshot> {
    match &workspace.provider_provisioning_snapshot {
        Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot,
        }) => endpoint_template_snapshot.clone(),
        None => None,
    }
}

pub(crate) fn endpoint_template_matches_workspace(
    template: &RunPodEndpointTemplateSnapshot,
    workspace: &Workspace,
) -> bool {
    template.provider_resource_status == ProviderResourceStatus::Ready
        && template.endpoint_worker_image_ref == workspace.resolved_runtime_image.endpoint_image_ref
        && template.mount_path
            == workspace
                .persistent_storage_volume_snapshot
                .as_ref()
                .map(|volume| volume.mount_path.clone())
                .unwrap_or_default()
}

pub(crate) fn is_workspace_ready(workspace: &Workspace) -> bool {
    workspace.environment_prepared_at.is_some()
        && workspace.active_provisioning_pod_snapshot.is_none()
        && workspace
            .persistent_storage_volume_snapshot
            .as_ref()
            .is_some_and(|snapshot| {
                snapshot.provider_resource_status == ProviderResourceStatus::Ready
            })
        && runpod_template_snapshot(workspace)
            .as_ref()
            .is_some_and(|snapshot| endpoint_template_matches_workspace(snapshot, workspace))
        && workspace
            .serverless_endpoint_snapshot
            .as_ref()
            .is_some_and(|snapshot| {
                snapshot.provider_resource_status == ProviderResourceStatus::Ready
            })
}

pub(crate) fn is_terminal_provider_resource_status(status: &ProviderResourceStatus) -> bool {
    matches!(
        status,
        ProviderResourceStatus::Failed
            | ProviderResourceStatus::Terminated
            | ProviderResourceStatus::Unknown
    )
}

pub(crate) fn progress_for_workspace(workspace: &Workspace) -> WorkspaceProvisioningProgress {
    match workspace.lifecycle_state {
        WorkspaceLifecycleState::Draft => WorkspaceProvisioningProgress {
            status: WorkspaceProvisioningStatus::Idle,
            phase: WorkspaceProvisioningPhase::NotStarted,
            percent: Some(0),
            failure: None,
        },
        WorkspaceLifecycleState::Provisioning => WorkspaceProvisioningProgress {
            status: WorkspaceProvisioningStatus::Running,
            phase: if workspace.persistent_storage_volume_snapshot.is_none() {
                WorkspaceProvisioningPhase::CreatingVolume
            } else if workspace.active_provisioning_pod_snapshot.is_none()
                && workspace.environment_prepared_at.is_none()
            {
                WorkspaceProvisioningPhase::StartingProvisioningPod
            } else if workspace.environment_prepared_at.is_none()
                || workspace.active_provisioning_pod_snapshot.is_some()
            {
                WorkspaceProvisioningPhase::PreparingEnvironment
            } else if runpod_template_snapshot(workspace).is_none() {
                WorkspaceProvisioningPhase::CreatingEndpointTemplate
            } else if workspace.serverless_endpoint_snapshot.is_none() {
                WorkspaceProvisioningPhase::CreatingEndpoint
            } else {
                WorkspaceProvisioningPhase::ValidatingReadiness
            },
            percent: None,
            failure: None,
        },
        WorkspaceLifecycleState::Ready => WorkspaceProvisioningProgress {
            status: WorkspaceProvisioningStatus::Completed,
            phase: WorkspaceProvisioningPhase::Completed,
            percent: Some(100),
            failure: None,
        },
        WorkspaceLifecycleState::Failed => WorkspaceProvisioningProgress {
            status: WorkspaceProvisioningStatus::Failed,
            phase: WorkspaceProvisioningPhase::Failed,
            percent: None,
            failure: Some(
                workspace
                    .last_provisioning_failure
                    .clone()
                    .unwrap_or_else(legacy_failure),
            ),
        },
    }
}

fn legacy_failure() -> WorkspaceProvisioningFailure {
    WorkspaceProvisioningFailure {
        code: WorkspaceProvisioningFailureCode::LegacyFailure,
        phase: WorkspaceProvisioningPhase::Failed,
        source: WorkspaceProvisioningFailureSource::Native,
        retryable: false,
        recovery_action: WorkspaceProvisioningRecoveryAction::InspectWorkspaceProvisioning,
        diagnostic: None,
    }
}
