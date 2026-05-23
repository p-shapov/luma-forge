use super::WorkspaceProvisioningError;
use crate::domain::workspace::{
    ProviderResourceStatus, Workspace, WorkspaceLifecycleState, WorkspaceProvisioningFailure,
    WorkspaceProvisioningFailureCode, WorkspaceProvisioningFailureSource,
    WorkspaceProvisioningPhase, WorkspaceProvisioningRecoveryAction,
};

use super::gateway::ProvisionerWorkerError;

pub(crate) fn fail_workspace(workspace: &mut Workspace, failure: WorkspaceProvisioningFailure) {
    workspace.lifecycle_state = WorkspaceLifecycleState::Failed;
    workspace.last_provisioning_failure = Some(failure);
}

pub(crate) fn provider_resource_failure(
    phase: WorkspaceProvisioningPhase,
    status: &ProviderResourceStatus,
) -> WorkspaceProvisioningFailure {
    WorkspaceProvisioningFailure {
        code: match status {
            ProviderResourceStatus::Failed => {
                WorkspaceProvisioningFailureCode::ProviderResourceFailed
            }
            ProviderResourceStatus::Terminated => {
                WorkspaceProvisioningFailureCode::ProviderResourceTerminated
            }
            ProviderResourceStatus::Unknown => {
                WorkspaceProvisioningFailureCode::ProviderResourceUnknown
            }
            ProviderResourceStatus::Creating
            | ProviderResourceStatus::Running
            | ProviderResourceStatus::Ready => {
                WorkspaceProvisioningFailureCode::ReadinessValidationFailed
            }
        },
        phase,
        source: WorkspaceProvisioningFailureSource::ProviderResource,
        recovery_action: WorkspaceProvisioningRecoveryAction::CleanupWorkspaceResources,
    }
}

pub(crate) fn indeterminate_provider_operation(
    phase: WorkspaceProvisioningPhase,
) -> WorkspaceProvisioningFailure {
    WorkspaceProvisioningFailure {
        code: WorkspaceProvisioningFailureCode::ProviderOperationIndeterminate,
        phase,
        source: WorkspaceProvisioningFailureSource::Provider,
        recovery_action: WorkspaceProvisioningRecoveryAction::CleanupWorkspaceResources,
    }
}

pub(crate) fn provisioning_error(
    phase: WorkspaceProvisioningPhase,
    error: &WorkspaceProvisioningError,
) -> Option<WorkspaceProvisioningFailure> {
    let (code, source, recovery_action) = match error {
        WorkspaceProvisioningError::ProviderSetupIncomplete => (
            WorkspaceProvisioningFailureCode::ProviderSetupIncomplete,
            WorkspaceProvisioningFailureSource::Native,
            WorkspaceProvisioningRecoveryAction::RecoverProviderSetup,
        ),
        WorkspaceProvisioningError::ProviderApiKeyUnauthorized => (
            WorkspaceProvisioningFailureCode::ProviderApiKeyUnauthorized,
            WorkspaceProvisioningFailureSource::Provider,
            WorkspaceProvisioningRecoveryAction::RecoverProviderSetup,
        ),
        WorkspaceProvisioningError::ProviderApiUnavailable => (
            WorkspaceProvisioningFailureCode::ProviderApiUnavailable,
            WorkspaceProvisioningFailureSource::Provider,
            WorkspaceProvisioningRecoveryAction::Retry,
        ),
        WorkspaceProvisioningError::ProviderRateLimited => (
            WorkspaceProvisioningFailureCode::ProviderRateLimited,
            WorkspaceProvisioningFailureSource::Provider,
            WorkspaceProvisioningRecoveryAction::Retry,
        ),
        WorkspaceProvisioningError::ProviderRequestRejected => (
            WorkspaceProvisioningFailureCode::ProviderRequestRejected,
            WorkspaceProvisioningFailureSource::Provider,
            WorkspaceProvisioningRecoveryAction::ReselectPlacement,
        ),
        WorkspaceProvisioningError::ProviderResponseInvalid => (
            WorkspaceProvisioningFailureCode::ProviderResponseInvalid,
            WorkspaceProvisioningFailureSource::Provider,
            WorkspaceProvisioningRecoveryAction::InspectWorkspaceProvisioning,
        ),
        WorkspaceProvisioningError::ProviderResourceNotFound => (
            WorkspaceProvisioningFailureCode::ProviderResourceMissing,
            WorkspaceProvisioningFailureSource::ProviderResource,
            WorkspaceProvisioningRecoveryAction::CleanupWorkspaceResources,
        ),
        WorkspaceProvisioningError::ProviderOperationConflict => (
            WorkspaceProvisioningFailureCode::ProviderOperationConflict,
            WorkspaceProvisioningFailureSource::Provider,
            WorkspaceProvisioningRecoveryAction::Retry,
        ),
        WorkspaceProvisioningError::ProviderOperationIndeterminate => (
            WorkspaceProvisioningFailureCode::ProviderOperationIndeterminate,
            WorkspaceProvisioningFailureSource::Provider,
            WorkspaceProvisioningRecoveryAction::CleanupWorkspaceResources,
        ),
        WorkspaceProvisioningError::ProvisionerWorkerTokenInvalid => (
            WorkspaceProvisioningFailureCode::ProvisionerWorkerTokenInvalid,
            WorkspaceProvisioningFailureSource::Native,
            WorkspaceProvisioningRecoveryAction::InspectWorkspaceProvisioning,
        ),
        WorkspaceProvisioningError::ProvisionerWorkerUnavailable => (
            WorkspaceProvisioningFailureCode::ProvisionerWorkerUnavailable,
            WorkspaceProvisioningFailureSource::ProvisionerWorker,
            WorkspaceProvisioningRecoveryAction::Retry,
        ),
        WorkspaceProvisioningError::WorkspaceNotFound
        | WorkspaceProvisioningError::InvalidWorkspaceLifecycle
        | WorkspaceProvisioningError::WorkspaceCatalogUnavailable
        | WorkspaceProvisioningError::WorkspaceCatalogStorageUnavailable
        | WorkspaceProvisioningError::WorkspaceCatalogMigrationFailed
        | WorkspaceProvisioningError::WorkspaceCatalogQueryFailed
        | WorkspaceProvisioningError::WorkspaceCatalogCorrupt
        | WorkspaceProvisioningError::WorkspaceCatalogSchemaMismatch
        | WorkspaceProvisioningError::SecureKeyringUnavailable
        | WorkspaceProvisioningError::ProvisionerWorkerConflict => return None,
        error => return worker_failure(phase, error),
    };

    Some(WorkspaceProvisioningFailure {
        code,
        phase,
        source,
        recovery_action,
    })
}

pub(crate) fn missing_provider_resource(
    phase: WorkspaceProvisioningPhase,
) -> WorkspaceProvisioningFailure {
    WorkspaceProvisioningFailure {
        code: WorkspaceProvisioningFailureCode::ProviderResourceMissing,
        phase,
        source: WorkspaceProvisioningFailureSource::ProviderResource,
        recovery_action: WorkspaceProvisioningRecoveryAction::CleanupWorkspaceResources,
    }
}

pub(crate) fn orphaned_provider_resources(
    phase: WorkspaceProvisioningPhase,
) -> WorkspaceProvisioningFailure {
    WorkspaceProvisioningFailure {
        code: WorkspaceProvisioningFailureCode::ProviderOrphanedResources,
        phase,
        source: WorkspaceProvisioningFailureSource::ProviderResource,
        recovery_action: WorkspaceProvisioningRecoveryAction::CleanupWorkspaceResources,
    }
}

pub(crate) fn worker_failure(
    phase: WorkspaceProvisioningPhase,
    error: &WorkspaceProvisioningError,
) -> Option<WorkspaceProvisioningFailure> {
    let code = match error {
        WorkspaceProvisioningError::ProvisionerWorkerUnauthorized => {
            WorkspaceProvisioningFailureCode::ProvisionerWorkerUnauthorized
        }
        WorkspaceProvisioningError::ProvisionerWorkerResponseInvalid => {
            WorkspaceProvisioningFailureCode::ProvisionerWorkerResponseInvalid
        }
        WorkspaceProvisioningError::ProvisionerWorkerFailed => {
            WorkspaceProvisioningFailureCode::ProvisionerWorkerFailed
        }
        _ => return None,
    };

    Some(WorkspaceProvisioningFailure {
        code,
        phase,
        source: WorkspaceProvisioningFailureSource::ProvisionerWorker,
        recovery_action: WorkspaceProvisioningRecoveryAction::InspectWorkspaceProvisioning,
    })
}

pub(crate) fn provisioner_worker_failure(
    phase: WorkspaceProvisioningPhase,
    error: &ProvisionerWorkerError,
) -> Option<WorkspaceProvisioningFailure> {
    let code = match error {
        ProvisionerWorkerError::Unauthorized => {
            WorkspaceProvisioningFailureCode::ProvisionerWorkerUnauthorized
        }
        ProvisionerWorkerError::InvalidPayload => {
            WorkspaceProvisioningFailureCode::ProvisionerWorkerResponseInvalid
        }
        ProvisionerWorkerError::Failed => WorkspaceProvisioningFailureCode::ProvisionerWorkerFailed,
        ProvisionerWorkerError::AssetDownloadFailed => {
            WorkspaceProvisioningFailureCode::ProvisionerWorkerAssetDownloadFailed
        }
        ProvisionerWorkerError::AssetAuthRequired => {
            WorkspaceProvisioningFailureCode::ProvisionerWorkerAssetAuthRequired
        }
        ProvisionerWorkerError::PathValidationFailed => {
            WorkspaceProvisioningFailureCode::ProvisionerWorkerPathValidationFailed
        }
        ProvisionerWorkerError::StepTimeout => {
            WorkspaceProvisioningFailureCode::ProvisionerWorkerStepTimeout
        }
        ProvisionerWorkerError::UnexpectedError => {
            WorkspaceProvisioningFailureCode::ProvisionerWorkerUnexpectedError
        }
        ProvisionerWorkerError::Conflict | ProvisionerWorkerError::Unreachable => return None,
    };

    Some(WorkspaceProvisioningFailure {
        code,
        phase,
        source: WorkspaceProvisioningFailureSource::ProvisionerWorker,
        recovery_action: WorkspaceProvisioningRecoveryAction::InspectWorkspaceProvisioning,
    })
}

pub(crate) fn worker_token_missing(
    phase: WorkspaceProvisioningPhase,
) -> WorkspaceProvisioningFailure {
    WorkspaceProvisioningFailure {
        code: WorkspaceProvisioningFailureCode::ProvisionerWorkerTokenMissing,
        phase,
        source: WorkspaceProvisioningFailureSource::Native,
        recovery_action: WorkspaceProvisioningRecoveryAction::InspectWorkspaceProvisioning,
    }
}

pub(crate) fn worker_token_invalid(
    phase: WorkspaceProvisioningPhase,
) -> WorkspaceProvisioningFailure {
    WorkspaceProvisioningFailure {
        code: WorkspaceProvisioningFailureCode::ProvisionerWorkerTokenInvalid,
        phase,
        source: WorkspaceProvisioningFailureSource::Native,
        recovery_action: WorkspaceProvisioningRecoveryAction::InspectWorkspaceProvisioning,
    }
}

pub(crate) fn readiness_validation_failed(
    phase: WorkspaceProvisioningPhase,
) -> WorkspaceProvisioningFailure {
    WorkspaceProvisioningFailure {
        code: WorkspaceProvisioningFailureCode::ReadinessValidationFailed,
        phase,
        source: WorkspaceProvisioningFailureSource::Native,
        recovery_action: WorkspaceProvisioningRecoveryAction::InspectWorkspaceProvisioning,
    }
}

pub(crate) fn cancellation_cleanup_failed() -> WorkspaceProvisioningFailure {
    WorkspaceProvisioningFailure {
        code: WorkspaceProvisioningFailureCode::CancellationCleanupFailed,
        phase: WorkspaceProvisioningPhase::CleaningUp,
        source: WorkspaceProvisioningFailureSource::Native,
        recovery_action: WorkspaceProvisioningRecoveryAction::CleanupWorkspaceResources,
    }
}
