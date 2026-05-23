use crate::domain::provider_setup::{GpuCloudProviderId, ProviderApiKey};

use super::{run_blocking_secret_operation, BlockingSecretStore, SecretStoreFuture};
use crate::secrets::SecretStoreError;

pub trait ProviderKeyStore: Send + Sync {
    fn has_api_key_entry(&self, provider_id: &GpuCloudProviderId)
        -> Result<bool, SecretStoreError>;

    fn read_api_key(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> Result<Option<ProviderApiKey>, SecretStoreError>;

    fn replace_api_key(
        &self,
        provider_id: &GpuCloudProviderId,
        api_key: &ProviderApiKey,
    ) -> Result<(), SecretStoreError>;

    fn delete_api_key(&self, provider_id: &GpuCloudProviderId) -> Result<(), SecretStoreError>;
}

pub trait AsyncProviderKeyStore: Send + Sync {
    fn has_api_key_entry<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> SecretStoreFuture<'a, bool>;

    fn read_api_key<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> SecretStoreFuture<'a, Option<ProviderApiKey>>;

    fn replace_api_key<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
        api_key: &'a ProviderApiKey,
    ) -> SecretStoreFuture<'a, ()>;

    fn delete_api_key<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> SecretStoreFuture<'a, ()>;
}

impl<S> AsyncProviderKeyStore for BlockingSecretStore<S>
where
    S: ProviderKeyStore + Clone + Send + Sync + 'static,
{
    fn has_api_key_entry<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> SecretStoreFuture<'a, bool> {
        let store = self.store.clone();
        let provider_id = *provider_id;
        Box::pin(async move {
            run_blocking_secret_operation(move || store.has_api_key_entry(&provider_id)).await
        })
    }

    fn read_api_key<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> SecretStoreFuture<'a, Option<ProviderApiKey>> {
        let store = self.store.clone();
        let provider_id = *provider_id;
        Box::pin(async move {
            run_blocking_secret_operation(move || store.read_api_key(&provider_id)).await
        })
    }

    fn replace_api_key<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
        api_key: &'a ProviderApiKey,
    ) -> SecretStoreFuture<'a, ()> {
        let store = self.store.clone();
        let provider_id = *provider_id;
        let api_key = api_key.clone();
        Box::pin(async move {
            run_blocking_secret_operation(move || store.replace_api_key(&provider_id, &api_key))
                .await
        })
    }

    fn delete_api_key<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> SecretStoreFuture<'a, ()> {
        let store = self.store.clone();
        let provider_id = *provider_id;
        Box::pin(async move {
            run_blocking_secret_operation(move || store.delete_api_key(&provider_id)).await
        })
    }
}

#[cfg(test)]
impl<S> AsyncProviderKeyStore for S
where
    S: ProviderKeyStore + Send + Sync,
{
    fn has_api_key_entry<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> SecretStoreFuture<'a, bool> {
        Box::pin(async move { ProviderKeyStore::has_api_key_entry(self, provider_id) })
    }

    fn read_api_key<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> SecretStoreFuture<'a, Option<ProviderApiKey>> {
        Box::pin(async move { ProviderKeyStore::read_api_key(self, provider_id) })
    }

    fn replace_api_key<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
        api_key: &'a ProviderApiKey,
    ) -> SecretStoreFuture<'a, ()> {
        Box::pin(async move { ProviderKeyStore::replace_api_key(self, provider_id, api_key) })
    }

    fn delete_api_key<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> SecretStoreFuture<'a, ()> {
        Box::pin(async move { ProviderKeyStore::delete_api_key(self, provider_id) })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    fn provider_key(value: &str) -> ProviderApiKey {
        ProviderApiKey::new(value.to_string()).expect("provider key should be valid")
    }

    #[tokio::test]
    async fn blocking_provider_key_store_runs_reads_off_async_worker_thread() {
        use std::thread::ThreadId;

        #[derive(Clone)]
        struct RecordingStore {
            caller_thread: ThreadId,
            observed_thread: Arc<Mutex<Option<ThreadId>>>,
        }

        impl ProviderKeyStore for RecordingStore {
            fn has_api_key_entry(
                &self,
                _provider_id: &GpuCloudProviderId,
            ) -> Result<bool, SecretStoreError> {
                Ok(false)
            }

            fn read_api_key(
                &self,
                _provider_id: &GpuCloudProviderId,
            ) -> Result<Option<ProviderApiKey>, SecretStoreError> {
                let observed = std::thread::current().id();
                assert_ne!(observed, self.caller_thread);
                *self.observed_thread.lock().expect("observed thread") = Some(observed);
                Ok(Some(provider_key("rp_secret")))
            }

            fn replace_api_key(
                &self,
                _provider_id: &GpuCloudProviderId,
                _api_key: &ProviderApiKey,
            ) -> Result<(), SecretStoreError> {
                Ok(())
            }

            fn delete_api_key(
                &self,
                _provider_id: &GpuCloudProviderId,
            ) -> Result<(), SecretStoreError> {
                Ok(())
            }
        }

        let observed_thread = Arc::new(Mutex::new(None));
        let store = BlockingSecretStore::new(RecordingStore {
            caller_thread: std::thread::current().id(),
            observed_thread: Arc::clone(&observed_thread),
        });

        let api_key = AsyncProviderKeyStore::read_api_key(&store, &GpuCloudProviderId::Runpod)
            .await
            .expect("async read should succeed")
            .expect("provider key should exist");

        assert_eq!(api_key.expose_secret(), "rp_secret");
        assert!(observed_thread.lock().expect("observed thread").is_some());
    }

    #[tokio::test]
    async fn blocking_provider_key_store_delegates_operations_and_preserves_errors() {
        #[derive(Clone)]
        struct ProviderOperationStore {
            result: Result<(), SecretStoreError>,
        }

        impl ProviderKeyStore for ProviderOperationStore {
            fn has_api_key_entry(
                &self,
                _provider_id: &GpuCloudProviderId,
            ) -> Result<bool, SecretStoreError> {
                self.result.clone()?;
                Ok(true)
            }

            fn read_api_key(
                &self,
                _provider_id: &GpuCloudProviderId,
            ) -> Result<Option<ProviderApiKey>, SecretStoreError> {
                self.result.clone()?;
                Ok(Some(provider_key("rp_secret")))
            }

            fn replace_api_key(
                &self,
                _provider_id: &GpuCloudProviderId,
                _api_key: &ProviderApiKey,
            ) -> Result<(), SecretStoreError> {
                self.result.clone()
            }

            fn delete_api_key(
                &self,
                _provider_id: &GpuCloudProviderId,
            ) -> Result<(), SecretStoreError> {
                self.result.clone()
            }
        }

        let store = BlockingSecretStore::new(ProviderOperationStore {
            result: Err(SecretStoreError::InvalidStoredProviderApiKey),
        });

        assert_eq!(
            AsyncProviderKeyStore::has_api_key_entry(&store, &GpuCloudProviderId::Runpod).await,
            Err(SecretStoreError::InvalidStoredProviderApiKey)
        );
        assert_eq!(
            AsyncProviderKeyStore::read_api_key(&store, &GpuCloudProviderId::Runpod)
                .await
                .map(|_| ()),
            Err(SecretStoreError::InvalidStoredProviderApiKey)
        );
        assert_eq!(
            AsyncProviderKeyStore::replace_api_key(
                &store,
                &GpuCloudProviderId::Runpod,
                &provider_key("rp_secret"),
            )
            .await,
            Err(SecretStoreError::InvalidStoredProviderApiKey)
        );
        assert_eq!(
            AsyncProviderKeyStore::delete_api_key(&store, &GpuCloudProviderId::Runpod).await,
            Err(SecretStoreError::InvalidStoredProviderApiKey)
        );
    }

    #[tokio::test]
    async fn blocking_provider_key_store_maps_blocking_task_panics_to_keyring_unavailable() {
        #[derive(Clone)]
        struct PanickingStore;

        impl ProviderKeyStore for PanickingStore {
            fn has_api_key_entry(
                &self,
                _provider_id: &GpuCloudProviderId,
            ) -> Result<bool, SecretStoreError> {
                panic!("blocking task failed");
            }

            fn read_api_key(
                &self,
                _provider_id: &GpuCloudProviderId,
            ) -> Result<Option<ProviderApiKey>, SecretStoreError> {
                Ok(None)
            }

            fn replace_api_key(
                &self,
                _provider_id: &GpuCloudProviderId,
                _api_key: &ProviderApiKey,
            ) -> Result<(), SecretStoreError> {
                Ok(())
            }

            fn delete_api_key(
                &self,
                _provider_id: &GpuCloudProviderId,
            ) -> Result<(), SecretStoreError> {
                Ok(())
            }
        }

        let store = BlockingSecretStore::new(PanickingStore);

        assert_eq!(
            AsyncProviderKeyStore::has_api_key_entry(&store, &GpuCloudProviderId::Runpod).await,
            Err(SecretStoreError::SecureKeyringUnavailable)
        );
    }
}
