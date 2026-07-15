#[derive(Debug, thiserror::Error)]
pub enum BundledCatalogError {
    #[error("bundled catalog io error at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("bundled catalog json error at {path}: {source}")]
    Json {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("bundled catalog contract error at {path}: {message}")]
    Contract { path: String, message: String },
    #[error("bundled catalog entry error at {path}: {message}")]
    Entry { path: String, message: String },
}
