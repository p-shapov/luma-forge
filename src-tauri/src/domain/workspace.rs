use serde::{Deserialize, Serialize};
use std::fmt;

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

impl WorkspaceState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotProvisioned => "not_provisioned",
            Self::Provisioning => "provisioning",
            Self::Ready => "ready",
            Self::CleaningUp => "cleaning_up",
            Self::Invalid => "invalid",
        }
    }
}

impl fmt::Display for WorkspaceState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
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
