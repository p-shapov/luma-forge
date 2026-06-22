use serde::{Deserialize, Serialize};

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    thiserror::Error,
    luma_diagnostic::DiagnosticCode,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCatalogError {
    #[error("workflow catalog parse failed: {message}")]
    ParseFailed { message: String },
    #[error("workflow catalog validation failed: {message}")]
    ValidationFailed { message: String },
}
