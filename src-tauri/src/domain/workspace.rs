use serde::{Deserialize, Serialize};

use super::{runpod::runtime::RunpodRuntime, workflow_preset::WorkflowReference};

pub type WorkspaceId = String;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceState {
    NotProvisioned,
    Provisioning,
    Ready,
    CleaningUp,
    Invalid,
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
