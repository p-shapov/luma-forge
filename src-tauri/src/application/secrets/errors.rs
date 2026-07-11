#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SecretsError {
    #[error("secret is already configured")]
    AlreadyConfigured,
    #[error("secret is not configured")]
    NotConfigured,
    #[error("credential is invalid")]
    InvalidCredential,
    #[error("identity provider is unavailable")]
    IdentityUnavailable,
    #[error("secret storage is unavailable")]
    StorageUnavailable,
}
