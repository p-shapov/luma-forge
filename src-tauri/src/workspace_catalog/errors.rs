#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceCatalogError {
    StorageUnavailable { message: String },
    SchemaInvalid { message: String },
    DataInvalid { message: String },
    WorkspaceAlreadyExists,
    WorkspaceNotFound,
}
