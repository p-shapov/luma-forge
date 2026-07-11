use secrecy::SecretString;

use crate::application::secrets::SecretKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SecretStoreError {
    #[error("secret already exists")]
    AlreadyExists,
    #[error("secret not found")]
    NotFound,
    #[error("secret storage is unavailable")]
    Unavailable,
}

#[async_trait::async_trait]
pub trait SecretStore: Send + Sync {
    async fn exists(&self, kind: SecretKind) -> Result<bool, SecretStoreError>;
    async fn get(&self, kind: SecretKind) -> Result<Option<SecretString>, SecretStoreError>;
    async fn insert(&self, kind: SecretKind, secret: SecretString) -> Result<(), SecretStoreError>;
    async fn delete(&self, kind: SecretKind) -> Result<(), SecretStoreError>;
}
