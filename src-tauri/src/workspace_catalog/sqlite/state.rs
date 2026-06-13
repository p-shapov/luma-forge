use crate::domain::workspace::WorkspaceState;

use crate::workspace_catalog::errors::{data_invalid_message, WorkspaceCatalogError};

pub(super) struct WorkspaceStateColumns {
    pub state: &'static str,
}

pub(super) fn workspace_state_columns(state: &WorkspaceState) -> WorkspaceStateColumns {
    match state {
        WorkspaceState::NotProvisioned => WorkspaceStateColumns {
            state: "not_provisioned",
        },
        WorkspaceState::Ready => WorkspaceStateColumns { state: "ready" },
        WorkspaceState::CleanupRequired => WorkspaceStateColumns {
            state: "cleanup_required",
        },
        WorkspaceState::Invalid => WorkspaceStateColumns { state: "invalid" },
    }
}

pub(super) fn workspace_state_from_columns(
    state: &str,
) -> Result<WorkspaceState, WorkspaceCatalogError> {
    match state {
        "not_provisioned" => Ok(WorkspaceState::NotProvisioned),
        "ready" => Ok(WorkspaceState::Ready),
        "cleanup_required" => Ok(WorkspaceState::CleanupRequired),
        "invalid" => Ok(WorkspaceState::Invalid),
        state => Err(data_invalid_message(format!("unknown state: {state}"))),
    }
}
