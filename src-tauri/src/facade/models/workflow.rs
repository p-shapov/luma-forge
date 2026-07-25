use serde::{Deserialize, Serialize};

use crate::application::runtimes::{CatalogRef, WorkflowSummary};

#[derive(
    luma_diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowPageDto {
    pub workflows: Vec<WorkflowDto>,
    pub total: u64,
}

#[derive(
    luma_diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct CatalogRefDto {
    #[diagnostic(show)]
    pub id: String,
    #[diagnostic(show)]
    pub revision: String,
}

impl From<CatalogRef> for CatalogRefDto {
    fn from(value: CatalogRef) -> Self {
        Self {
            id: value.id,
            revision: value.revision,
        }
    }
}

impl From<CatalogRefDto> for CatalogRef {
    fn from(value: CatalogRefDto) -> Self {
        Self::new(value.id, value.revision)
    }
}

#[derive(
    luma_diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowDto {
    pub id: String,
    pub revision: String,
    pub name: String,
    pub description: String,
    pub required_volume_size_gb: u64,
    pub requires_hugging_face_api_key: bool,
}

impl From<WorkflowSummary> for WorkflowDto {
    fn from(value: WorkflowSummary) -> Self {
        Self {
            id: value.id,
            revision: value.revision,
            name: value.name,
            description: value.description,
            required_volume_size_gb: value.required_volume_size_gb,
            requires_hugging_face_api_key: value.requires_hugging_face_api_key,
        }
    }
}
