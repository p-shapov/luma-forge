#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum KeyringStorageError {
    #[error("secure storage is unavailable")]
    Unavailable,
}
