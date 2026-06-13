use crate::{domain::secrets::ApiKeyIdentity, shared::AppFuture};

use super::{errors::SecretsStorageError, stores::ApiSecret};

pub trait ApiKeyIdentityProvider: Send + Sync {
    fn identity<'a>(
        &'a self,
        secret: &'a ApiSecret,
    ) -> AppFuture<'a, Result<ApiKeyIdentity, SecretsStorageError>>;
}
