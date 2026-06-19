use crate::{domain::secrets::ApiKeyIdentity, shared::AppFuture};
use hmac::{Hmac, Mac};
use sha2::Sha256;

use super::{ApiSecret, SecretKey, SecretsStorageError};

pub trait ApiKeyIdentityProvider: Send + Sync {
    fn identity<'a>(
        &'a self,
        secret: &'a ApiSecret,
    ) -> AppFuture<'a, Result<ApiKeyIdentity, SecretsStorageError>>;
}

pub trait SecretStore: Send + Sync {
    fn has<'a>(&'a self, key: SecretKey) -> AppFuture<'a, Result<bool, SecretsStorageError>>;

    fn write<'a>(
        &'a self,
        key: SecretKey,
        secret: ApiSecret,
    ) -> AppFuture<'a, Result<(), SecretsStorageError>>;

    fn delete<'a>(&'a self, key: SecretKey) -> AppFuture<'a, Result<(), SecretsStorageError>>;

    fn read<'a>(
        &'a self,
        key: SecretKey,
    ) -> AppFuture<'a, Result<Option<ApiSecret>, SecretsStorageError>>;
}

#[derive(Clone)]
pub struct SecretsService<S, I> {
    store: S,
    identity: I,
    key: SecretKey,
}

impl<S, I> SecretsService<S, I> {
    pub fn new(store: S, identity: I, key: SecretKey) -> Self {
        Self {
            store,
            identity,
            key,
        }
    }
}

impl<S, I> SecretsService<S, I>
where
    S: SecretStore,
    I: ApiKeyIdentityProvider,
{
    pub async fn write(&self, secret: ApiSecret) -> Result<ApiKeyIdentity, SecretsStorageError> {
        if self.store.has(self.key).await? {
            return Err(SecretsStorageError::KeyAlreadyExists);
        }

        let identity = self.identity.identity(&secret).await?;
        self.store.write(self.key, secret).await?;

        Ok(identity)
    }

    pub async fn identity(&self) -> Result<ApiKeyIdentity, SecretsStorageError> {
        let secret = self.stored_secret().await?;

        self.identity.identity(&secret).await
    }

    pub async fn retrieve(&self) -> Result<ApiSecret, SecretsStorageError> {
        self.stored_secret().await
    }

    pub async fn hmac_sha256_hex(&self, message: &str) -> Result<String, SecretsStorageError> {
        let secret = self.stored_secret().await?;
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.expose_secret().as_bytes())
            .map_err(|_| SecretsStorageError::StoreUnavailable)?;

        mac.update(message.as_bytes());
        Ok(hex::encode(mac.finalize().into_bytes()))
    }

    pub async fn remove(&self) -> Result<(), SecretsStorageError> {
        if !self.store.has(self.key).await? {
            return Err(SecretsStorageError::KeyNotFound);
        }

        self.store.delete(self.key).await
    }

    async fn stored_secret(&self) -> Result<ApiSecret, SecretsStorageError> {
        self.store
            .read(self.key)
            .await?
            .ok_or(SecretsStorageError::KeyNotFound)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, VecDeque},
        sync::{Arc, Mutex},
    };

    use crate::{
        domain::secrets::ApiKeyIdentity,
        shared::{ApiError, AppFuture},
    };

    use super::*;
    use crate::secrets::{
        ApiKeyIdentityProvider, ApiSecret, SecretKey, SecretStore, SecretsStorageError,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum StoreCall {
        Has(SecretKey),
        Write(SecretKey),
        Read(SecretKey),
        Delete(SecretKey),
    }

    #[derive(Clone, Default)]
    struct FakeStore {
        inner: Arc<Mutex<FakeStoreState>>,
    }

    #[derive(Default)]
    struct FakeStoreState {
        secrets: HashMap<SecretKey, ApiSecret>,
        calls: Vec<StoreCall>,
    }

    impl FakeStore {
        fn insert(&self, key: SecretKey, secret: ApiSecret) {
            self.inner
                .lock()
                .expect("store state")
                .secrets
                .insert(key, secret);
        }

        fn calls(&self) -> Vec<StoreCall> {
            self.inner.lock().expect("store state").calls.clone()
        }

        fn secret(&self, key: SecretKey) -> Option<String> {
            self.inner
                .lock()
                .expect("store state")
                .secrets
                .get(&key)
                .map(|secret| secret.expose_secret().to_string())
        }
    }

    impl SecretStore for FakeStore {
        fn has<'a>(&'a self, key: SecretKey) -> AppFuture<'a, Result<bool, SecretsStorageError>> {
            Box::pin(async move {
                let mut state = self.inner.lock().expect("store state");
                state.calls.push(StoreCall::Has(key));

                Ok(state.secrets.contains_key(&key))
            })
        }

        fn write<'a>(
            &'a self,
            key: SecretKey,
            secret: ApiSecret,
        ) -> AppFuture<'a, Result<(), SecretsStorageError>> {
            Box::pin(async move {
                let mut state = self.inner.lock().expect("store state");
                state.calls.push(StoreCall::Write(key));
                state.secrets.insert(key, secret);

                Ok(())
            })
        }

        fn delete<'a>(&'a self, key: SecretKey) -> AppFuture<'a, Result<(), SecretsStorageError>> {
            Box::pin(async move {
                let mut state = self.inner.lock().expect("store state");
                state.calls.push(StoreCall::Delete(key));
                state.secrets.remove(&key);

                Ok(())
            })
        }

        fn read<'a>(
            &'a self,
            key: SecretKey,
        ) -> AppFuture<'a, Result<Option<ApiSecret>, SecretsStorageError>> {
            Box::pin(async move {
                let mut state = self.inner.lock().expect("store state");
                state.calls.push(StoreCall::Read(key));

                Ok(state.secrets.get(&key).cloned())
            })
        }
    }

    #[derive(Clone)]
    struct FakeIdentityProvider {
        inner: Arc<Mutex<FakeIdentityState>>,
    }

    struct FakeIdentityState {
        calls: Vec<String>,
        results: VecDeque<Result<ApiKeyIdentity, SecretsStorageError>>,
    }

    impl FakeIdentityProvider {
        fn new(results: Vec<Result<ApiKeyIdentity, SecretsStorageError>>) -> Self {
            Self {
                inner: Arc::new(Mutex::new(FakeIdentityState {
                    calls: Vec::new(),
                    results: VecDeque::from(results),
                })),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.inner.lock().expect("identity state").calls.clone()
        }
    }

    impl ApiKeyIdentityProvider for FakeIdentityProvider {
        fn identity<'a>(
            &'a self,
            secret: &'a ApiSecret,
        ) -> AppFuture<'a, Result<ApiKeyIdentity, SecretsStorageError>> {
            Box::pin(async move {
                let mut state = self.inner.lock().expect("identity state");
                state.calls.push(secret.expose_secret().to_string());

                state.results.pop_front().expect("identity result")
            })
        }
    }

    fn identity() -> ApiKeyIdentity {
        ApiKeyIdentity {
            email: Some("user@example.com".to_string()),
            username: Some("user".to_string()),
            key_display_name: Some("key".to_string()),
        }
    }

    fn secret(value: &str) -> ApiSecret {
        ApiSecret::new(value.to_string()).expect("secret")
    }

    #[tokio::test]
    async fn write_rejects_existing_key_without_validation_or_write() {
        let store = FakeStore::default();
        store.insert(SecretKey::RunpodApiKey, secret("existing"));
        let identity = FakeIdentityProvider::new(vec![Ok(identity())]);
        let service = SecretsService::new(store.clone(), identity.clone(), SecretKey::RunpodApiKey);

        let result = service.write(secret("replacement")).await;

        assert_eq!(result, Err(SecretsStorageError::KeyAlreadyExists));
        assert_eq!(store.calls(), vec![StoreCall::Has(SecretKey::RunpodApiKey)]);
        assert_eq!(identity.calls(), Vec::<String>::new());
        assert_eq!(
            store.secret(SecretKey::RunpodApiKey),
            Some("existing".to_string())
        );
    }

    #[tokio::test]
    async fn write_validates_before_storing_and_returns_identity() {
        let store = FakeStore::default();
        let expected_identity = identity();
        let identity = FakeIdentityProvider::new(vec![Ok(expected_identity.clone())]);
        let service = SecretsService::new(store.clone(), identity.clone(), SecretKey::RunpodApiKey);

        let result = service.write(secret("runpod-secret")).await;

        assert_eq!(result, Ok(expected_identity));
        assert_eq!(
            store.calls(),
            vec![
                StoreCall::Has(SecretKey::RunpodApiKey),
                StoreCall::Write(SecretKey::RunpodApiKey)
            ]
        );
        assert_eq!(identity.calls(), vec!["runpod-secret".to_string()]);
        assert_eq!(
            store.secret(SecretKey::RunpodApiKey),
            Some("runpod-secret".to_string())
        );
    }

    #[tokio::test]
    async fn write_does_not_store_after_validation_failure() {
        let store = FakeStore::default();
        let identity = FakeIdentityProvider::new(vec![Err(
            SecretsStorageError::IdentityRequestFailed(ApiError::Unauthorized),
        )]);
        let service = SecretsService::new(store.clone(), identity.clone(), SecretKey::RunpodApiKey);

        let result = service.write(secret("bad-secret")).await;

        assert_eq!(
            result,
            Err(SecretsStorageError::IdentityRequestFailed(
                ApiError::Unauthorized
            ))
        );
        assert_eq!(store.calls(), vec![StoreCall::Has(SecretKey::RunpodApiKey)]);
        assert_eq!(identity.calls(), vec!["bad-secret".to_string()]);
        assert_eq!(store.secret(SecretKey::RunpodApiKey), None);
    }

    #[tokio::test]
    async fn identity_reads_stored_secret_and_validates_it() {
        let store = FakeStore::default();
        store.insert(SecretKey::RunpodApiKey, secret("stored-secret"));
        let expected_identity = identity();
        let identity = FakeIdentityProvider::new(vec![Ok(expected_identity.clone())]);
        let service = SecretsService::new(store.clone(), identity.clone(), SecretKey::RunpodApiKey);

        let result = service.identity().await;

        assert_eq!(result, Ok(expected_identity));
        assert_eq!(
            store.calls(),
            vec![StoreCall::Read(SecretKey::RunpodApiKey)]
        );
        assert_eq!(identity.calls(), vec!["stored-secret".to_string()]);
    }

    #[tokio::test]
    async fn identity_returns_not_found_when_secret_missing() {
        let store = FakeStore::default();
        let identity = FakeIdentityProvider::new(vec![Ok(identity())]);
        let service = SecretsService::new(store.clone(), identity.clone(), SecretKey::RunpodApiKey);

        let result = service.identity().await;

        assert_eq!(result, Err(SecretsStorageError::KeyNotFound));
        assert_eq!(
            store.calls(),
            vec![StoreCall::Read(SecretKey::RunpodApiKey)]
        );
        assert_eq!(identity.calls(), Vec::<String>::new());
    }

    #[tokio::test]
    async fn retrieve_returns_stored_secret_without_validation() {
        let store = FakeStore::default();
        store.insert(SecretKey::RunpodApiKey, secret("stored-secret"));
        let identity = FakeIdentityProvider::new(vec![Ok(identity())]);
        let service = SecretsService::new(store.clone(), identity.clone(), SecretKey::RunpodApiKey);

        let result = service.retrieve().await;

        assert_eq!(
            result.map(|secret| secret.expose_secret().to_string()),
            Ok("stored-secret".to_string())
        );
        assert_eq!(
            store.calls(),
            vec![StoreCall::Read(SecretKey::RunpodApiKey)]
        );
        assert_eq!(identity.calls(), Vec::<String>::new());
    }

    #[tokio::test]
    async fn retrieve_returns_not_found_when_secret_missing() {
        let store = FakeStore::default();
        let identity = FakeIdentityProvider::new(vec![Ok(identity())]);
        let service = SecretsService::new(store.clone(), identity.clone(), SecretKey::RunpodApiKey);

        let result = service.retrieve().await;

        assert_eq!(result.map(|_| ()), Err(SecretsStorageError::KeyNotFound));
        assert_eq!(
            store.calls(),
            vec![StoreCall::Read(SecretKey::RunpodApiKey)]
        );
        assert_eq!(identity.calls(), Vec::<String>::new());
    }

    #[tokio::test]
    async fn hmac_sha256_hex_returns_lowercase_hex_digest() {
        let store = FakeStore::default();
        store.insert(SecretKey::RunpodApiKey, secret("secret"));
        let identity = FakeIdentityProvider::new(vec![]);
        let service = SecretsService::new(store.clone(), identity, SecretKey::RunpodApiKey);

        let digest = service
            .hmac_sha256_hex("workspace-1")
            .await
            .expect("digest should be returned");

        assert_eq!(
            digest,
            "3a30dee134306692b7fc538c583821a2a5f9f5bc57b89c74a6e1148a177817f1"
        );
        assert_eq!(digest.len(), 64);
        assert!(digest
            .chars()
            .all(|character| character.is_ascii_hexdigit()));
        assert_eq!(
            store.calls(),
            vec![StoreCall::Read(SecretKey::RunpodApiKey)]
        );
    }

    #[tokio::test]
    async fn hmac_sha256_hex_returns_key_not_found_when_secret_missing() {
        let store = FakeStore::default();
        let identity = FakeIdentityProvider::new(vec![]);
        let service = SecretsService::new(store, identity, SecretKey::RunpodApiKey);

        let result = service.hmac_sha256_hex("workspace-1").await;

        assert_eq!(result, Err(SecretsStorageError::KeyNotFound));
    }

    #[tokio::test]
    async fn remove_deletes_existing_secret() {
        let store = FakeStore::default();
        store.insert(SecretKey::RunpodApiKey, secret("stored-secret"));
        let identity = FakeIdentityProvider::new(vec![Ok(identity())]);
        let service = SecretsService::new(store.clone(), identity.clone(), SecretKey::RunpodApiKey);

        let result = service.remove().await;

        assert_eq!(result, Ok(()));
        assert_eq!(
            store.calls(),
            vec![
                StoreCall::Has(SecretKey::RunpodApiKey),
                StoreCall::Delete(SecretKey::RunpodApiKey)
            ]
        );
        assert_eq!(identity.calls(), Vec::<String>::new());
        assert_eq!(store.secret(SecretKey::RunpodApiKey), None);
    }

    #[tokio::test]
    async fn remove_returns_not_found_when_secret_missing() {
        let store = FakeStore::default();
        let identity = FakeIdentityProvider::new(vec![Ok(identity())]);
        let service = SecretsService::new(store.clone(), identity.clone(), SecretKey::RunpodApiKey);

        let result = service.remove().await;

        assert_eq!(result, Err(SecretsStorageError::KeyNotFound));
        assert_eq!(store.calls(), vec![StoreCall::Has(SecretKey::RunpodApiKey)]);
        assert_eq!(identity.calls(), Vec::<String>::new());
    }
}
