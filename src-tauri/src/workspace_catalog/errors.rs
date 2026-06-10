#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceCatalogError {
    StorageUnavailable,
    MigrationFailed,
    QueryFailed,
    Corrupt,
    SchemaMismatch,
    WorkspaceAlreadyExists,
    WorkspaceNotFound,
}
