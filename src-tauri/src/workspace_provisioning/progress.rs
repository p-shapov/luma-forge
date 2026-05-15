use crate::domain::workspace::{
    Workspace, WorkspaceLifecycleState, WorkspaceProvisioningFailure,
    WorkspaceProvisioningFailureCode, WorkspaceProvisioningFailureSource,
    WorkspaceProvisioningPhase, WorkspaceProvisioningProgress, WorkspaceProvisioningRecoveryAction,
    WorkspaceProvisioningStatus,
};

use super::{contracts::WorkspaceProvisioningResult, snapshots::runpod_template_snapshot};

pub(crate) fn result(workspace: Workspace) -> WorkspaceProvisioningResult {
    let progress = progress_for_workspace(&workspace);
    WorkspaceProvisioningResult {
        workspace,
        progress,
    }
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
