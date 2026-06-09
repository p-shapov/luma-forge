use crate::{
    domain::{
        lifecycle_operation::{LifecycleOperationPayload, WorkspaceId},
        provisioned_remote::{
            ProvisionedRemoteLifecycleError, ProvisionedRemoteLifecycleOperationPayload,
            ProvisionedRemoteResources,
        },
        workspace::{
            WorkspaceCleanupRequiredReason, WorkspaceRuntimeInvalidReason, WorkspaceState,
        },
    },
    lifecycle_journal::LifecycleJournalError,
};

use super::super::errors::ProvisionedRemoteError;

pub fn map_lifecycle_journal_error(
    error: LifecycleJournalError,
    workspace_id: &WorkspaceId,
) -> ProvisionedRemoteError {
    match error {
        LifecycleJournalError::RunningOperationExists => {
            ProvisionedRemoteError::LifecycleOperationAlreadyRunning {
                workspace_id: workspace_id.clone(),
            }
        }
        LifecycleJournalError::OperationNotFound
        | LifecycleJournalError::StorageUnavailable
        | LifecycleJournalError::QueryFailed
        | LifecycleJournalError::Corrupt
        | LifecycleJournalError::SchemaMismatch => ProvisionedRemoteError::StorageUnavailable,
    }
}

pub fn interrupted_state_for_resources(resources: &ProvisionedRemoteResources) -> WorkspaceState {
    if resources.is_empty() {
        WorkspaceState::Invalid {
            reason: WorkspaceRuntimeInvalidReason::OperationInterrupted,
        }
    } else {
        WorkspaceState::CleanupRequired {
            reason: WorkspaceCleanupRequiredReason::OperationInterrupted,
        }
    }
}

pub fn payload_with_app_interrupted_error(
    payload: &LifecycleOperationPayload,
) -> LifecycleOperationPayload {
    match payload {
        LifecycleOperationPayload::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayload::Provision { step, .. },
        ) => LifecycleOperationPayload::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayload::Provision {
                step: step.clone(),
                error: Some(ProvisionedRemoteLifecycleError::AppInterrupted),
            },
        ),
        LifecycleOperationPayload::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayload::Cleanup { step, .. },
        ) => LifecycleOperationPayload::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayload::Cleanup {
                step: step.clone(),
                error: Some(ProvisionedRemoteLifecycleError::AppInterrupted),
            },
        ),
        LifecycleOperationPayload::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayload::Delete { step, .. },
        ) => LifecycleOperationPayload::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayload::Delete {
                step: step.clone(),
                error: Some(ProvisionedRemoteLifecycleError::AppInterrupted),
            },
        ),
    }
}

pub fn is_delete_payload(payload: &LifecycleOperationPayload) -> bool {
    matches!(
        payload,
        LifecycleOperationPayload::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayload::Delete { .. }
        )
    )
}
