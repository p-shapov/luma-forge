#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BundledCatalogError {
    #[error("bundled catalog io error at {path}: {message}")]
    Io { path: String, message: String },
    #[error("bundled catalog json parse error at {path}: {message}")]
    JsonParse { path: String, message: String },
    #[error("bundled catalog schema error at {path}: {message}")]
    Schema { path: String, message: String },
    #[error("bundled catalog contract error at {path}: {message}")]
    Contract { path: String, message: String },
    #[error("bundled catalog unresolved reference at {path}: {entity}/{id}/{revision}")]
    UnresolvedReference {
        path: String,
        entity: String,
        id: String,
        revision: String,
    },
}
