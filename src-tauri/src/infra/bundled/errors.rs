#[derive(Debug, thiserror::Error)]
pub enum BundledCatalogError {
    #[error("bundled asset is corrupt: {path}: {message}")]
    CorruptBundledAsset { path: String, message: String },
}

impl BundledCatalogError {
    pub(crate) fn corrupt_asset(path: &str, message: impl Into<String>) -> Self {
        Self::CorruptBundledAsset {
            path: path.to_string(),
            message: message.into(),
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum BundledValidationError {
    #[error("{path}: {message}")]
    Invalid { path: String, message: String },
}
