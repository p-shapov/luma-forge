#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorkflowCatalogError {
    #[error("workflow catalog parse failed: {message}")]
    ParseFailed { message: String },
    #[error("workflow catalog validation failed: {message}")]
    ValidationFailed { message: String },
}
