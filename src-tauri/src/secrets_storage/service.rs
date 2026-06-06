use crate::domain::secrets::ApiKeyIdentity;

use super::{ApiKeyIdentityProvider, ApiSecret, SecretKey, SecretStore, SecretsStorageError};

pub struct SecretsStorageService<S, I> {
    store: S,
    identity: I,
}

impl<S, I> SecretsStorageService<S, I> {
    pub fn new(store: S, identity: I) -> Self {
        Self { store, identity }
    }
}

impl<S, I> SecretsStorageService<S, I>
where
    S: SecretStore,
    I: ApiKeyIdentityProvider,
{
    pub async fn write(
        &self,
        key: SecretKey,
        secret: ApiSecret,
    ) -> Result<ApiKeyIdentity, SecretsStorageError> {
        if self.store.has(key).await? {
            return Err(SecretsStorageError::KeyAlreadyExists);
        }

        let identity = self.identity.identity(&secret).await?;
        self.store.write(key, secret).await?;

        Ok(identity)
    }

    pub async fn identity(&self, key: SecretKey) -> Result<ApiKeyIdentity, SecretsStorageError> {
        let secret = self
            .store
            .read(key)
            .await?
            .ok_or(SecretsStorageError::KeyNotFound)?;

        self.identity.identity(&secret).await
    }

    pub async fn retrieve(&self, key: SecretKey) -> Result<ApiSecret, SecretsStorageError> {
        self.store
            .read(key)
            .await?
            .ok_or(SecretsStorageError::KeyNotFound)
    }

    pub async fn remove(&self, key: SecretKey) -> Result<(), SecretsStorageError> {
        if !self.store.has(key).await? {
            return Err(SecretsStorageError::KeyNotFound);
        }

        self.store.delete(key).await
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, VecDeque},
        sync::{Arc, Mutex},
    };

    use crate::{
        domain::{provider::ProviderError, secrets::ApiKeyIdentity},
        shared::AppFuture,
    };

    use super::*;
    use crate::secrets_storage::{
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
        let service = SecretsStorageService::new(store.clone(), identity.clone());

        let result = service
            .write(SecretKey::RunpodApiKey, secret("replacement"))
            .await;

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
        let service = SecretsStorageService::new(store.clone(), identity.clone());

        let result = service
            .write(SecretKey::RunpodApiKey, secret("runpod-secret"))
            .await;

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
        let identity = FakeIdentityProvider::new(vec![Err(ProviderError::Unauthorized.into())]);
        let service = SecretsStorageService::new(store.clone(), identity.clone());

        let result = service
            .write(SecretKey::RunpodApiKey, secret("bad-secret"))
            .await;

        assert_eq!(result, Err(ProviderError::Unauthorized.into()));
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
        let service = SecretsStorageService::new(store.clone(), identity.clone());

        let result = service.identity(SecretKey::RunpodApiKey).await;

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
        let service = SecretsStorageService::new(store.clone(), identity.clone());

        let result = service.identity(SecretKey::RunpodApiKey).await;

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
        let service = SecretsStorageService::new(store.clone(), identity.clone());

        let result = service.retrieve(SecretKey::RunpodApiKey).await;

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
        let service = SecretsStorageService::new(store.clone(), identity.clone());

        let result = service.retrieve(SecretKey::RunpodApiKey).await;

        assert_eq!(result.map(|_| ()), Err(SecretsStorageError::KeyNotFound));
        assert_eq!(
            store.calls(),
            vec![StoreCall::Read(SecretKey::RunpodApiKey)]
        );
        assert_eq!(identity.calls(), Vec::<String>::new());
    }

    #[tokio::test]
    async fn remove_deletes_existing_secret() {
        let store = FakeStore::default();
        store.insert(SecretKey::RunpodApiKey, secret("stored-secret"));
        let identity = FakeIdentityProvider::new(vec![Ok(identity())]);
        let service = SecretsStorageService::new(store.clone(), identity.clone());

        let result = service.remove(SecretKey::RunpodApiKey).await;

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
        let service = SecretsStorageService::new(store.clone(), identity.clone());

        let result = service.remove(SecretKey::RunpodApiKey).await;

        assert_eq!(result, Err(SecretsStorageError::KeyNotFound));
        assert_eq!(store.calls(), vec![StoreCall::Has(SecretKey::RunpodApiKey)]);
        assert_eq!(identity.calls(), Vec::<String>::new());
    }
}
