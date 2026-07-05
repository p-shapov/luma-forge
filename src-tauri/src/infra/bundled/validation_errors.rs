#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum BundledValidationError {
    #[error("{path}: {message}")]
    Invalid { path: String, message: String },
}
