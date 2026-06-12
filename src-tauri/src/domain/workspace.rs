use serde::{Deserialize, Serialize};

use super::{runpod_runtime::RunpodRuntime, workflow_preset::WorkflowReference};

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceCleanupRequiredReason {
    ProvisionFailed,
    CleanupFailed,
    DeleteFailed,
    OperationInterrupted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceRuntimeInvalidReason {
    OperationInterrupted,
    ProvisionFailed,
    CleanupFailed,
    DeleteFailed,
    CorruptRuntimeState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "runtime_type", rename_all = "snake_case")]
pub enum WorkspaceRuntime {
    Runpod(RunpodRuntime),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub workflow: WorkflowReference,
    pub state: WorkspaceState,
    pub runtime: WorkspaceRuntime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceCatalog {
    pub workspaces: Vec<Workspace>,
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        runpod_runtime::{RunpodPlacementPlan, RunpodResources, RunpodRuntime},
        workflow_preset::WorkflowReference,
    };

    use super::{Workspace, WorkspaceRuntime, WorkspaceState};

    #[test]
    fn workspace_serializes_stable_state_separately_from_runtime() {
        let workspace = Workspace {
            id: "workspace-1".to_string(),
            workflow: workflow_reference(),
            state: WorkspaceState::NotProvisioned,
            runtime: WorkspaceRuntime::Runpod(RunpodRuntime {
                placement: runpod_placement(),
                resources: RunpodResources {
                    network_volume_id: None,
                    provisioner_pod_id: None,
                    endpoint_id: Some("endpoint".to_string()),
                    template_id: Some("template".to_string()),
                },
            }),
        };

        let json = serde_json::to_value(&workspace).expect("workspace should serialize");

        assert_eq!(json["state"], "not_provisioned");
        assert_eq!(json["runtime"]["runtime_type"], "runpod");
        assert!(json["runtime"]["resources"].is_object());
        assert_eq!(json["runtime"]["resources"]["endpoint_id"], "endpoint");
        assert_eq!(json["runtime"]["resources"]["template_id"], "template");
        assert!(json["runtime"]["resources"].get("endpoint").is_none());
        assert!(json["runtime"]["placement"]
            .get("gpu_cloud_provider_id")
            .is_none());
        assert_eq!(json["runtime"]["placement"]["volume_size_gb"], 19);
        assert_eq!(json["workflow"]["id"], "preset");
        assert_eq!(json["workflow"]["version"], "1.0.0");
        assert!(json.get("workflow_preset").is_none());
        assert_eq!(
            json["runtime"]
                .as_object()
                .expect("runtime should be object")
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec![
                "placement".to_string(),
                "resources".to_string(),
                "runtime_type".to_string()
            ]
        );
    }

    fn runpod_placement() -> RunpodPlacementPlan {
        RunpodPlacementPlan {
            data_center_id: "dc".to_string(),
            gpu_type_id: "gpu".to_string(),
            volume_size_gb: 19,
            keep_alive_limits: None,
        }
    }

    fn workflow_reference() -> WorkflowReference {
        WorkflowReference {
            id: "preset".to_string(),
            version: "1.0.0".to_string(),
        }
    }
}
