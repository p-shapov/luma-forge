use serde::{Deserialize, Serialize};

use crate::application::workspace::Workspace;

use super::{
    mapping::timestamp, CatalogRefDto, FacadeMappingError, RuntimeDto, RuntimeOperationDto,
};

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePageDto {
    pub workspaces: Vec<WorkspaceDto>,
    pub total: u64,
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkspaceRequest {
    #[diagnostic(show)]
    pub workflow: CatalogRefDto,
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceIdRequest {
    #[diagnostic(show)]
    pub workspace_id: String,
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionWorkspaceRequest {
    #[diagnostic(show)]
    pub workspace_id: String,
    #[diagnostic(show)]
    pub runtime: ProvisionRuntimeInput,
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(
    tag = "runtimeKind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ProvisionRuntimeInput {
    Runpod {
        #[diagnostic(show)]
        datacenter_id: String,
        #[diagnostic(show)]
        gpu_id: String,
        #[diagnostic(show)]
        volume_size_gb: u64,
    },
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceOperationDto {
    pub workspace: WorkspaceDto,
    pub operation: RuntimeOperationDto,
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDto {
    pub id: String,
    pub workflow: CatalogRefDto,
    pub created_at: String,
    pub runtime: Option<RuntimeDto>,
}

impl TryFrom<Workspace> for WorkspaceDto {
    type Error = FacadeMappingError;

    fn try_from(value: Workspace) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            workflow: value.workflow.into(),
            created_at: timestamp(value.created_at)?,
            runtime: value.runtime.map(Into::into),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "workspace_changed")]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceChangedEvent {
    pub workspace: WorkspaceDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "workspace_deleted")]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDeletedEvent {
    pub workspace_id: String,
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use crate::application::runtimes::{
        runpod::{RunpodRuntime, RunpodRuntimeConfig, RunpodRuntimeResources},
        CatalogRef, Runtime, RuntimeProvider, RuntimeState,
    };

    use super::*;

    fn workspace_with_runpod_resources() -> Workspace {
        Workspace {
            id: "workspace-1".into(),
            workflow: CatalogRef::new("workflow-1", "1"),
            created_at: OffsetDateTime::UNIX_EPOCH,
            runtime: Some(Runtime {
                state: RuntimeState::Ready,
                provider: RuntimeProvider::Runpod(RunpodRuntime {
                    config: RunpodRuntimeConfig {
                        datacenter_id: "EU-RO-1".into(),
                        gpu_id: "gpu-1".into(),
                        volume_size_gb: 100,
                    },
                    resources: RunpodRuntimeResources {
                        endpoint_id: Some("endpoint-1".into()),
                        ..Default::default()
                    },
                }),
            }),
        }
    }

    #[test]
    fn workspace_dto_exposes_shared_state_but_omits_provider_resource_ids() {
        let dto = WorkspaceDto::try_from(workspace_with_runpod_resources()).unwrap();
        let json = serde_json::to_value(dto).unwrap();
        assert_eq!(json["runtime"]["state"], "ready");
        assert_eq!(json["runtime"]["provider"]["runtimeKind"], "runpod");
        assert_eq!(json["runtime"]["provider"]["volumeSizeGb"], 100);
        assert!(json["runtime"]["provider"].get("volume_size_gb").is_none());
        assert!(json["runtime"]["provider"].get("resources").is_none());
        assert!(!json.to_string().contains("endpoint-1"));
    }
}
