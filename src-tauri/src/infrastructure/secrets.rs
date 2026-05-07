use std::{future::Future, pin::Pin, sync::Arc};

use keyring::Entry;
use secrecy::{ExposeSecret, SecretString};

use crate::domain::provider_setup::{GpuCloudProviderId, ProviderSetupError};

pub(crate) const KEYRING_SERVICE: &str = "com.lumaforge.provider-api-key";

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub(crate) trait ProviderApiKeyStore: Send + Sync {
    fn get<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> BoxFuture<'a, Result<Option<SecretString>, ProviderSetupError>>;

    fn contains<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> BoxFuture<'a, Result<bool, ProviderSetupError>>;

    fn set<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
        api_key: SecretString,
    ) -> BoxFuture<'a, Result<(), ProviderSetupError>>;

    fn delete<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> BoxFuture<'a, Result<(), ProviderSetupError>>;
}

#[derive(Clone)]
pub(crate) struct SharedProviderApiKeyStore(Arc<dyn ProviderApiKeyStore>);

impl SharedProviderApiKeyStore {
    pub(crate) fn new(store: impl ProviderApiKeyStore + 'static) -> Self {
        Self(Arc::new(store))
    }
}

impl ProviderApiKeyStore for SharedProviderApiKeyStore {
    fn get<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> BoxFuture<'a, Result<Option<SecretString>, ProviderSetupError>> {
        self.0.get(provider_id)
    }

    fn contains<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> BoxFuture<'a, Result<bool, ProviderSetupError>> {
        self.0.contains(provider_id)
    }

    fn set<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
        api_key: SecretString,
    ) -> BoxFuture<'a, Result<(), ProviderSetupError>> {
        self.0.set(provider_id, api_key)
    }

    fn delete<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> BoxFuture<'a, Result<(), ProviderSetupError>> {
        self.0.delete(provider_id)
    }
}

#[derive(Clone)]
pub(crate) struct KeyringProviderApiKeyStore {
    service: String,
}

impl KeyringProviderApiKeyStore {
    pub(crate) fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    fn entry(&self, provider_id: &GpuCloudProviderId) -> Result<Entry, ProviderSetupError> {
        Entry::new(&self.service, provider_id.as_str())
            .map_err(|_| ProviderSetupError::SecureKeyringUnavailable)
    }
}

impl ProviderApiKeyStore for KeyringProviderApiKeyStore {
    fn get<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> BoxFuture<'a, Result<Option<SecretString>, ProviderSetupError>> {
        Box::pin(async move {
            match self.entry(provider_id)?.get_password() {
                Ok(api_key) => Ok(Some(SecretString::from(api_key))),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(_) => Err(ProviderSetupError::SecureKeyringUnavailable),
            }
        })
    }

    fn contains<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> BoxFuture<'a, Result<bool, ProviderSetupError>> {
        Box::pin(async move { self.get(provider_id).await.map(|value| value.is_some()) })
    }

    fn set<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
        api_key: SecretString,
    ) -> BoxFuture<'a, Result<(), ProviderSetupError>> {
        Box::pin(async move {
            self.entry(provider_id)?
                .set_password(api_key.expose_secret())
                .map_err(|_| ProviderSetupError::SecureKeyringUnavailable)
        })
    }

    fn delete<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> BoxFuture<'a, Result<(), ProviderSetupError>> {
        Box::pin(async move {
            match self.entry(provider_id)?.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(_) => Err(ProviderSetupError::SecureKeyringUnavailable),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
    };

    use tokio::sync::Mutex;

    use super::*;

    #[derive(Clone, Default)]
    struct FakeProviderApiKeyStore {
        keys: Arc<Mutex<HashMap<String, SecretString>>>,
        deletes: Arc<AtomicUsize>,
    }

    impl ProviderApiKeyStore for FakeProviderApiKeyStore {
        fn get<'a>(
            &'a self,
            provider_id: &'a GpuCloudProviderId,
        ) -> BoxFuture<'a, Result<Option<SecretString>, ProviderSetupError>> {
            Box::pin(async move { Ok(self.keys.lock().await.get(provider_id.as_str()).cloned()) })
        }

        fn contains<'a>(
            &'a self,
            provider_id: &'a GpuCloudProviderId,
        ) -> BoxFuture<'a, Result<bool, ProviderSetupError>> {
            Box::pin(async move { Ok(self.keys.lock().await.contains_key(provider_id.as_str())) })
        }

        fn set<'a>(
            &'a self,
            provider_id: &'a GpuCloudProviderId,
            api_key: SecretString,
        ) -> BoxFuture<'a, Result<(), ProviderSetupError>> {
            Box::pin(async move {
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
        ) -> BoxFuture<'a, Result<(), ProviderSetupError>> {
            Box::pin(async move {
                self.deletes.fetch_add(1, Ordering::SeqCst);
                self.keys.lock().await.remove(provider_id.as_str());
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn fake_store_reads_writes_and_checks_presence() {
        let store = FakeProviderApiKeyStore::default();

        assert!(!store
            .contains(&GpuCloudProviderId::RunPod)
            .await
            .expect("presence check should succeed"));

        store
            .set(
                &GpuCloudProviderId::RunPod,
                SecretString::from("secret".to_owned()),
            )
            .await
            .expect("write should succeed");

        assert!(store
            .contains(&GpuCloudProviderId::RunPod)
            .await
            .expect("presence check should succeed"));
        assert!(store
            .get(&GpuCloudProviderId::RunPod)
            .await
            .expect("read should succeed")
            .is_some());
    }

    #[tokio::test]
    async fn fake_store_deletes_existing_key_for_rollback() {
        let store = FakeProviderApiKeyStore::default();
        store
            .set(
                &GpuCloudProviderId::RunPod,
                SecretString::from("secret".to_owned()),
            )
            .await
            .expect("write should succeed");

        store
            .delete(&GpuCloudProviderId::RunPod)
            .await
            .expect("delete should succeed");

        assert_eq!(store.deletes.load(Ordering::SeqCst), 1);
        assert!(!store
            .contains(&GpuCloudProviderId::RunPod)
            .await
            .expect("presence check should succeed"));
    }
}
