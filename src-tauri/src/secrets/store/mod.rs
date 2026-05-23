pub mod provider_key;
pub mod provisioner_token;

use std::{future::Future, pin::Pin};

use super::SecretStoreError;

pub type SecretStoreFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, SecretStoreError>> + Send + 'a>>;

#[derive(Debug, Clone)]
pub struct BlockingSecretStore<S> {
    pub(super) store: S,
}

impl<S> BlockingSecretStore<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

pub(super) async fn run_blocking_secret_operation<T>(
    operation: impl FnOnce() -> Result<T, SecretStoreError> + Send + 'static,
) -> Result<T, SecretStoreError>
where
    T: Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|_| SecretStoreError::SecureKeyringUnavailable)?
}
