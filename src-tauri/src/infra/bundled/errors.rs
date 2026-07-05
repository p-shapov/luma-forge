#[derive(Debug, thiserror::Error)]
pub enum BundledCatalogError {
    #[error("bundled asset is corrupt: {path}: {message}")]
    CorruptBundledAsset { path: String, message: String },
}
