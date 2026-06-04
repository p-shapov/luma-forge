#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowCatalogError {
    ParseFailed,
    ValidationFailed,
}
