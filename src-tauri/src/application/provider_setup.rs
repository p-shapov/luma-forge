use std::sync::Arc;

use secrecy::SecretString;
use tokio::sync::Mutex;

use crate::{
    domain::provider_setup::{
        GpuCloudProviderId, GpuCloudProviderSetup, ProviderSetupError, ProviderSetupMetadata,
    },
    infrastructure::{
        provider_setup_repository::{ProviderSetupRepository, SharedProviderSetupRepository},
        providers::{GpuProvider, GpuProviderRegistry},
        secrets::{ProviderApiKeyStore, SharedProviderApiKeyStore},
    },
};

#[derive(Clone)]
pub(crate) struct ProviderSetupService {
    repository: SharedProviderSetupRepository,
    key_store: SharedProviderApiKeyStore,
    provider_registry: Arc<GpuProviderRegistry>,
    mutation_lock: Arc<Mutex<()>>,
}

impl ProviderSetupService {
    pub(crate) fn new(
        repository: impl ProviderSetupRepository + 'static,
        key_store: impl ProviderApiKeyStore + 'static,
        provider_registry: GpuProviderRegistry,
    ) -> Self {
        Self {
            repository: SharedProviderSetupRepository::new(repository),
            key_store: SharedProviderApiKeyStore::new(key_store),
            provider_registry: Arc::new(provider_registry),
            mutation_lock: Arc::new(Mutex::new(())),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_shared_parts(
        repository: SharedProviderSetupRepository,
        key_store: SharedProviderApiKeyStore,
        provider_registry: GpuProviderRegistry,
    ) -> Self {
        Self {
            repository,
            key_store,
            provider_registry: Arc::new(provider_registry),
            mutation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub(crate) async fn get_setup(
        &self,
        provider_id: GpuCloudProviderId,
    ) -> Result<Option<GpuCloudProviderSetup>, ProviderSetupError> {
        self.read_local_complete(&provider_id).await
    }

    pub(crate) async fn sync_setup(
        &self,
        provider_id: GpuCloudProviderId,
    ) -> Result<GpuCloudProviderSetup, ProviderSetupError> {
        let _guard = self.mutation_lock.lock().await;

        let api_key = self
            .key_store
            .get(&provider_id)
            .await?
            .ok_or(ProviderSetupError::ProviderSetupIncomplete)?;
        let provider = self.provider_registry.provider(&provider_id)?;
        let validated = provider.validate_api_key(api_key).await?;
        let metadata = ProviderSetupMetadata {
            provider_id: provider_id.clone(),
            provider_user_id: validated.provider_user_id,
            provider_api_key_fingerprint: validated.provider_api_key_fingerprint,
        };

        self.repository.save(metadata).await?;
        self.read_local_complete(&provider_id)
            .await?
            .ok_or(ProviderSetupError::LocalStorageUnavailable)
    }

    pub(crate) async fn setup_provider(
        &self,
        provider_id: GpuCloudProviderId,
        provider_api_key: String,
    ) -> Result<GpuCloudProviderSetup, ProviderSetupError> {
        if provider_api_key.trim().is_empty() {
            return Err(ProviderSetupError::InvalidProviderApiKey);
        }

        let _guard = self.mutation_lock.lock().await;

        if self.read_local_complete(&provider_id).await?.is_some() {
            return Err(ProviderSetupError::ProviderSetupAlreadyExists);
        }

        let api_key = SecretString::from(provider_api_key);
        let provider = self.provider_registry.provider(&provider_id)?;
        let validated = provider.validate_api_key(api_key.clone()).await?;

        self.key_store.set(&provider_id, api_key).await?;

        let metadata = ProviderSetupMetadata {
            provider_id: provider_id.clone(),
            provider_user_id: validated.provider_user_id,
            provider_api_key_fingerprint: validated.provider_api_key_fingerprint,
        };

        if self.repository.save(metadata).await.is_err() {
            let _ = self.key_store.delete(&provider_id).await;
            return Err(ProviderSetupError::LocalStorageUnavailable);
        }

        self.read_local_complete(&provider_id)
            .await?
            .ok_or(ProviderSetupError::LocalStorageUnavailable)
    }

    pub(crate) async fn delete_setup(
        &self,
        provider_id: GpuCloudProviderId,
    ) -> Result<Option<GpuCloudProviderSetup>, ProviderSetupError> {
        let _guard = self.mutation_lock.lock().await;

        self.key_store.delete(&provider_id).await?;
        self.repository.delete(&provider_id).await?;

        Ok(None)
    }

    async fn read_local_complete(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> Result<Option<GpuCloudProviderSetup>, ProviderSetupError> {
        let metadata = self.repository.get(provider_id).await?;
        let key_present = self.key_store.contains(provider_id).await?;

        match (metadata, key_present) {
            (Some(metadata), true) => Ok(Some(metadata.redacted_setup())),
            (Some(_), false) => Err(ProviderSetupError::ProviderSetupIncomplete),
            (None, _) => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        future::Future,
        pin::Pin,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
    };

    use secrecy::SecretString;
    use tokio::sync::Mutex as AsyncMutex;

    use super::*;
    use crate::{
        domain::provider_setup::ValidatedProviderCredential,
        infrastructure::{
            provider_setup_repository::SharedProviderSetupRepository, providers::GpuProvider,
            secrets::SharedProviderApiKeyStore,
        },
    };

    type TestFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

    #[derive(Clone, Default)]
    struct FakeRepository {
        metadata: Arc<AsyncMutex<HashMap<String, ProviderSetupMetadata>>>,
        fail_save: Arc<AsyncMutex<bool>>,
    }

    impl FakeRepository {
        async fn set_fail_save(&self, value: bool) {
            *self.fail_save.lock().await = value;
        }
    }

    impl ProviderSetupRepository for FakeRepository {
        fn get<'a>(
            &'a self,
            provider_id: &'a GpuCloudProviderId,
        ) -> TestFuture<'a, Result<Option<ProviderSetupMetadata>, ProviderSetupError>> {
            Box::pin(async move {
                Ok(self
                    .metadata
                    .lock()
                    .await
                    .get(provider_id.as_str())
                    .cloned())
            })
        }

        fn save<'a>(
            &'a self,
            metadata: ProviderSetupMetadata,
        ) -> TestFuture<'a, Result<(), ProviderSetupError>> {
            Box::pin(async move {
                if *self.fail_save.lock().await {
                    return Err(ProviderSetupError::LocalStorageUnavailable);
                }

                self.metadata
                    .lock()
                    .await
                    .insert(metadata.provider_id.as_str().to_owned(), metadata);
                Ok(())
            })
        }

        fn delete<'a>(
            &'a self,
            provider_id: &'a GpuCloudProviderId,
        ) -> TestFuture<'a, Result<(), ProviderSetupError>> {
            Box::pin(async move {
                self.metadata.lock().await.remove(provider_id.as_str());
                Ok(())
            })
        }
    }

    #[derive(Clone, Default)]
    struct FakeKeyStore {
        keys: Arc<AsyncMutex<HashMap<String, SecretString>>>,
        writes: Arc<AtomicUsize>,
        deletes: Arc<AtomicUsize>,
    }

    impl ProviderApiKeyStore for FakeKeyStore {
        fn get<'a>(
            &'a self,
            provider_id: &'a GpuCloudProviderId,
        ) -> TestFuture<'a, Result<Option<SecretString>, ProviderSetupError>> {
            Box::pin(async move { Ok(self.keys.lock().await.get(provider_id.as_str()).cloned()) })
        }

        fn contains<'a>(
            &'a self,
            provider_id: &'a GpuCloudProviderId,
        ) -> TestFuture<'a, Result<bool, ProviderSetupError>> {
            Box::pin(async move { Ok(self.keys.lock().await.contains_key(provider_id.as_str())) })
        }

        fn set<'a>(
            &'a self,
            provider_id: &'a GpuCloudProviderId,
            api_key: SecretString,
        ) -> TestFuture<'a, Result<(), ProviderSetupError>> {
            Box::pin(async move {
                self.writes.fetch_add(1, Ordering::SeqCst);
                self.keys
                    .lock()
                    .await
                    .insert(provider_id.as_str().to_owned(), api_key);
                Ok(())
            })
        }

        fn delete<'a>(
            &'a self,
            provider_id: &'a GpuCloudProviderId,
        ) -> TestFuture<'a, Result<(), ProviderSetupError>> {
            Box::pin(async move {
                self.deletes.fetch_add(1, Ordering::SeqCst);
                self.keys.lock().await.remove(provider_id.as_str());
                Ok(())
            })
        }
    }

    #[derive(Clone)]
    struct FakeProvider {
        result: Arc<AsyncMutex<Result<ValidatedProviderCredential, ProviderSetupError>>>,
        calls: Arc<AtomicUsize>,
    }

    impl FakeProvider {
        fn valid() -> Self {
            Self {
                result: Arc::new(AsyncMutex::new(Ok(ValidatedProviderCredential {
                    provider_user_id: "user-123".to_owned(),
                    provider_api_key_fingerprint: "rpa_key_id".to_owned(),
                }))),
                calls: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    impl GpuProvider for FakeProvider {
        fn validate_api_key<'a>(
            &'a self,
            _api_key: SecretString,
        ) -> TestFuture<'a, Result<ValidatedProviderCredential, ProviderSetupError>> {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::SeqCst);
                self.result.lock().await.clone()
            })
        }
    }

    fn service(
        repository: FakeRepository,
        key_store: FakeKeyStore,
        provider: FakeProvider,
    ) -> ProviderSetupService {
        ProviderSetupService::with_shared_parts(
            SharedProviderSetupRepository::new(repository),
            SharedProviderApiKeyStore::new(key_store),
            GpuProviderRegistry::new(provider),
        )
    }

    #[tokio::test]
    async fn setup_provider_saves_key_and_metadata() {
        let repository = FakeRepository::default();
        let key_store = FakeKeyStore::default();
        let service = service(repository, key_store, FakeProvider::valid());

        let setup = service
            .setup_provider(GpuCloudProviderId::RunPod, "rpa_key_id_secret".to_owned())
            .await
            .expect("setup should succeed");

        assert_eq!(setup.gpu_cloud_provider_id, GpuCloudProviderId::RunPod);
        assert_eq!(setup.provider_user_id, "user-123");
        assert_eq!(setup.provider_api_key_fingerprint, "rpa_key_id");
    }

    #[tokio::test]
    async fn setup_rejects_empty_key() {
        let service = service(
            FakeRepository::default(),
            FakeKeyStore::default(),
            FakeProvider::valid(),
        );

        let error = service
            .setup_provider(GpuCloudProviderId::RunPod, " ".to_owned())
            .await
            .expect_err("empty key should fail");

        assert!(matches!(error, ProviderSetupError::InvalidProviderApiKey));
    }

    #[tokio::test]
    async fn setup_rejects_invalid_provider_key_without_mutation() {
        let repository = FakeRepository::default();
        let key_store = FakeKeyStore::default();
        let provider = FakeProvider {
            result: Arc::new(AsyncMutex::new(Err(
                ProviderSetupError::InvalidProviderApiKey,
            ))),
            calls: Arc::new(AtomicUsize::new(0)),
        };
        let service = service(repository, key_store.clone(), provider);

        let error = service
            .setup_provider(GpuCloudProviderId::RunPod, "secret".to_owned())
            .await
            .expect_err("invalid key should fail");

        assert!(matches!(error, ProviderSetupError::InvalidProviderApiKey));
        assert_eq!(key_store.writes.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn setup_rejects_existing_complete_setup() {
        let repository = FakeRepository::default();
        let key_store = FakeKeyStore::default();
        let service = service(repository, key_store, FakeProvider::valid());

        service
            .setup_provider(GpuCloudProviderId::RunPod, "rpa_key_id_secret".to_owned())
            .await
            .expect("first setup should succeed");
        let error = service
            .setup_provider(GpuCloudProviderId::RunPod, "rpa_key_id_secret".to_owned())
            .await
            .expect_err("second setup should fail");

        assert!(matches!(
            error,
            ProviderSetupError::ProviderSetupAlreadyExists
        ));
    }

    #[tokio::test]
    async fn setup_rolls_back_key_when_metadata_save_fails() {
        let repository = FakeRepository::default();
        repository.set_fail_save(true).await;
        let key_store = FakeKeyStore::default();
        let service = service(repository, key_store.clone(), FakeProvider::valid());

        let error = service
            .setup_provider(GpuCloudProviderId::RunPod, "rpa_key_id_secret".to_owned())
            .await
            .expect_err("metadata failure should fail setup");

        assert!(matches!(error, ProviderSetupError::LocalStorageUnavailable));
        assert_eq!(key_store.deletes.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn get_setup_does_not_recover_missing_metadata_from_existing_key() {
        let repository = FakeRepository::default();
        let key_store = FakeKeyStore::default();
        key_store
            .set(
                &GpuCloudProviderId::RunPod,
                SecretString::from("rpa_key_id_secret".to_owned()),
            )
            .await
            .expect("fake key set should succeed");
        let provider = FakeProvider::valid();
        let calls = provider.calls.clone();
        let service = service(repository, key_store, provider);

        let setup = service
            .get_setup(GpuCloudProviderId::RunPod)
            .await
            .expect("local read should succeed");

        assert!(setup.is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn sync_setup_recovers_missing_metadata_from_existing_key() {
        let repository = FakeRepository::default();
        let key_store = FakeKeyStore::default();
        key_store
            .set(
                &GpuCloudProviderId::RunPod,
                SecretString::from("rpa_key_id_secret".to_owned()),
            )
            .await
            .expect("fake key set should succeed");
        let provider = FakeProvider::valid();
        let calls = provider.calls.clone();
        let service = service(repository, key_store, provider);

        let setup = service
            .sync_setup(GpuCloudProviderId::RunPod)
            .await
            .expect("sync should succeed");

        assert_eq!(setup.provider_api_key_fingerprint, "rpa_key_id");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn sync_setup_rejects_missing_key() {
        let service = service(
            FakeRepository::default(),
            FakeKeyStore::default(),
            FakeProvider::valid(),
        );

        let error = service
            .sync_setup(GpuCloudProviderId::RunPod)
            .await
            .expect_err("missing key should fail sync");

        assert!(matches!(error, ProviderSetupError::ProviderSetupIncomplete));
    }

    #[tokio::test]
    async fn delete_setup_removes_key_and_metadata() {
        let repository = FakeRepository::default();
        let key_store = FakeKeyStore::default();
        let service = service(repository, key_store.clone(), FakeProvider::valid());

        service
            .setup_provider(GpuCloudProviderId::RunPod, "rpa_key_id_secret".to_owned())
            .await
            .expect("setup should succeed");
        let setup = service
            .delete_setup(GpuCloudProviderId::RunPod)
            .await
            .expect("delete should succeed");
        let status = service
            .get_setup(GpuCloudProviderId::RunPod)
            .await
            .expect("status read should succeed");

        assert!(setup.is_none());
        assert!(status.is_none());
        assert_eq!(key_store.deletes.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn delete_setup_is_idempotent_when_setup_is_absent() {
        let service = service(
            FakeRepository::default(),
            FakeKeyStore::default(),
            FakeProvider::valid(),
        );

        let setup = service
            .delete_setup(GpuCloudProviderId::RunPod)
            .await
            .expect("delete should succeed when setup is absent");

        assert!(setup.is_none());
    }

    #[tokio::test]
    async fn concurrent_setup_allows_only_one_completion() {
        let service = service(
            FakeRepository::default(),
            FakeKeyStore::default(),
            FakeProvider::valid(),
        );

        let first = service.setup_provider(GpuCloudProviderId::RunPod, "rpa_key_id_a".to_owned());
        let second =
            service.setup_provider(GpuCloudProviderId::RunPod, "rpa_key_id_b".to_owned());
        let (first, second) = tokio::join!(first, second);
        let successes = usize::from(first.is_ok()) + usize::from(second.is_ok());
        let failures = [first.err(), second.err()]
            .into_iter()
            .flatten()
            .filter(|error| matches!(error, ProviderSetupError::ProviderSetupAlreadyExists))
            .count();

        assert_eq!(successes, 1);
        assert_eq!(failures, 1);
    }
}
