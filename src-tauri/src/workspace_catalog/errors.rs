#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorkspaceCatalogError {
    #[error("workspace catalog storage unavailable: {message}")]
    StorageUnavailable { message: String },
    #[error("workspace catalog schema is invalid: {message}")]
    SchemaInvalid { message: String },
    #[error("workspace catalog data is invalid: {message}")]
    DataInvalid { message: String },
    #[error("workspace already exists")]
    WorkspaceAlreadyExists,
    #[error("workspace was not found")]
    WorkspaceNotFound,
}
