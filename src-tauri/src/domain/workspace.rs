use serde::{Deserialize, Serialize};

use crate::domain::runpod::RunpodLifecycleError;

use super::{runpod::runtime::RunpodRuntime, workflow_preset::WorkflowReference};

pub type WorkspaceId = String;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceState {
    NotProvisioned,
    Ready,
    CleanupRequired {
        reason: WorkspaceCleanupRequiredReason,
    },
    Invalid {
        reason: WorkspaceRuntimeInvalidReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceCleanupRequiredReason {
    #[error("provision failed")]
    ProvisionFailed,
    #[error("cleanup failed")]
    CleanupFailed,
    #[error("delete failed")]
    DeleteFailed,
    #[error("operation interrupted")]
    OperationInterrupted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceRuntimeInvalidReason {
    #[error("operation interrupted")]
    OperationInterrupted,
    #[error("provision failed")]
    ProvisionFailed,
    #[error("cleanup failed")]
    CleanupFailed,
    #[error("delete failed")]
    DeleteFailed,
    #[error("corrupt runtime state")]
    CorruptRuntimeState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "runtime_type", rename_all = "snake_case")]
pub enum WorkspaceRuntime {
    Runpod(RunpodRuntime),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub workflow: WorkflowReference,
    pub state: WorkspaceState,
    pub runtime: WorkspaceRuntime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceCatalog {
    pub workspaces: Vec<Workspace>,
}
