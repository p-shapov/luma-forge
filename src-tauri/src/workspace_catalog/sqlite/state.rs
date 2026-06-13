use crate::domain::workspace::{
    WorkspaceCleanupRequiredReason, WorkspaceRuntimeInvalidReason, WorkspaceState,
};

use crate::workspace_catalog::errors::{WorkspaceCatalogError, data_invalid_message};

pub(super) struct WorkspaceStateColumns {
    pub state: &'static str,
    pub reason: Option<&'static str>,
}

pub(super) fn workspace_state_columns(state: &WorkspaceState) -> WorkspaceStateColumns {
    match state {
        WorkspaceState::NotProvisioned => WorkspaceStateColumns {
            state: "not_provisioned",
            reason: None,
        },
        WorkspaceState::Ready => WorkspaceStateColumns {
            state: "ready",
            reason: None,
        },
        WorkspaceState::CleanupRequired { reason } => WorkspaceStateColumns {
            state: "cleanup_required",
            reason: Some(cleanup_required_reason_column(reason)),
        },
        WorkspaceState::Invalid { reason } => WorkspaceStateColumns {
            state: "invalid",
            reason: Some(invalid_reason_column(reason)),
        },
    }
}

pub(super) fn workspace_state_from_columns(
    state: &str,
    reason: Option<&str>,
) -> Result<WorkspaceState, WorkspaceCatalogError> {
    match (state, reason) {
        ("not_provisioned", None) => Ok(WorkspaceState::NotProvisioned),
        ("ready", None) => Ok(WorkspaceState::Ready),
        ("cleanup_required", Some(reason)) => Ok(WorkspaceState::CleanupRequired {
            reason: cleanup_required_reason_from_column(reason)?,
        }),
        ("invalid", Some(reason)) => Ok(WorkspaceState::Invalid {
            reason: invalid_reason_from_column(reason)?,
        }),
        (state, _) => Err(data_invalid_message(format!("unknown state: {state}"))),
    }
}

fn cleanup_required_reason_column(reason: &WorkspaceCleanupRequiredReason) -> &'static str {
    match reason {
        WorkspaceCleanupRequiredReason::ProvisionFailed => "provision_failed",
        WorkspaceCleanupRequiredReason::CleanupFailed => "cleanup_failed",
        WorkspaceCleanupRequiredReason::DeleteFailed => "delete_failed",
        WorkspaceCleanupRequiredReason::OperationInterrupted => "operation_interrupted",
    }
}

fn cleanup_required_reason_from_column(
    reason: &str,
) -> Result<WorkspaceCleanupRequiredReason, WorkspaceCatalogError> {
    match reason {
        "provision_failed" => Ok(WorkspaceCleanupRequiredReason::ProvisionFailed),
        "cleanup_failed" => Ok(WorkspaceCleanupRequiredReason::CleanupFailed),
        "delete_failed" => Ok(WorkspaceCleanupRequiredReason::DeleteFailed),
        "operation_interrupted" => Ok(WorkspaceCleanupRequiredReason::OperationInterrupted),
        reason => Err(data_invalid_message(format!(
            "unknown cleanup required reason: {reason}"
        ))),
    }
}

fn invalid_reason_column(reason: &WorkspaceRuntimeInvalidReason) -> &'static str {
    match reason {
        WorkspaceRuntimeInvalidReason::OperationInterrupted => "operation_interrupted",
        WorkspaceRuntimeInvalidReason::ProvisionFailed => "provision_failed",
        WorkspaceRuntimeInvalidReason::CleanupFailed => "cleanup_failed",
        WorkspaceRuntimeInvalidReason::DeleteFailed => "delete_failed",
        WorkspaceRuntimeInvalidReason::CorruptRuntimeState => "corrupt_runtime_state",
    }
}

fn invalid_reason_from_column(
    reason: &str,
) -> Result<WorkspaceRuntimeInvalidReason, WorkspaceCatalogError> {
    match reason {
        "operation_interrupted" => Ok(WorkspaceRuntimeInvalidReason::OperationInterrupted),
        "provision_failed" => Ok(WorkspaceRuntimeInvalidReason::ProvisionFailed),
        "cleanup_failed" => Ok(WorkspaceRuntimeInvalidReason::CleanupFailed),
        "delete_failed" => Ok(WorkspaceRuntimeInvalidReason::DeleteFailed),
        "corrupt_runtime_state" => Ok(WorkspaceRuntimeInvalidReason::CorruptRuntimeState),
        reason => Err(data_invalid_message(format!(
            "unknown invalid reason: {reason}"
        ))),
    }
}
