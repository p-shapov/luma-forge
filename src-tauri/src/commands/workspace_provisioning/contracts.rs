use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    domain::workspace as domain_workspace, workspace_provisioning::WorkspaceProvisioningResult,
};

#[allow(dead_code)]
mod remote_types {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workspace::WorkspaceProvisioningStatus)]
    #[serde(rename_all = "snake_case")]
    pub(super) enum WorkspaceProvisioningStatus {
        Idle,
        Running,
        Cancelling,
        Completed,
        Failed,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workspace::WorkspaceProvisioningPhase)]
    #[serde(rename_all = "snake_case")]
    pub(super) enum WorkspaceProvisioningPhase {
        NotStarted,
        CreatingVolume,
        StartingProvisioningPod,
        PreparingEnvironment,
        CreatingEndpointTemplate,
        CreatingEndpoint,
        ValidatingReadiness,
        CleaningUp,
        Completed,
        Failed,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workspace::WorkspaceProvisioningProgress)]
    pub(super) struct WorkspaceProvisioningProgress {
        pub status: domain_workspace::WorkspaceProvisioningStatus,
        pub phase: domain_workspace::WorkspaceProvisioningPhase,
        pub percent: Option<u8>,
        pub message: Option<String>,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct WorkspaceProvisioningRequest {
    pub workspace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct WorkspaceProvisioningResponse {
    pub workspace: domain_workspace::Workspace,
    pub progress: domain_workspace::WorkspaceProvisioningProgress,
}

impl From<WorkspaceProvisioningResult> for WorkspaceProvisioningResponse {
    fn from(result: WorkspaceProvisioningResult) -> Self {
        Self {
            workspace: result.workspace,
            progress: result.progress,
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
