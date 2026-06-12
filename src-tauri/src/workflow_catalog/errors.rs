#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowCatalogError {
    ParseFailed { message: String },
    ValidationFailed { message: String },
}
