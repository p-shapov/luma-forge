#[derive(Debug, thiserror::Error)]
pub enum BundledCatalogError {
    #[error("bundled asset is corrupt: {path}: {message}")]
    CorruptBundledAsset { path: String, message: String },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum BundledValidationError {
    #[error("{path}: {message}")]
    Invalid { path: String, message: String },
}
