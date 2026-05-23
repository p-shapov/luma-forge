use crate::domain::hugging_face_setup::HuggingFaceApiKey;

use super::{run_blocking_secret_operation, BlockingSecretStore, SecretStoreFuture};
use crate::secrets::SecretStoreError;

pub trait HuggingFaceApiKeyStore: Send + Sync {
    fn has_hugging_face_api_key_entry(&self) -> Result<bool, SecretStoreError>;

    fn read_hugging_face_api_key(&self) -> Result<Option<HuggingFaceApiKey>, SecretStoreError>;

    fn replace_hugging_face_api_key(
        &self,
        api_key: &HuggingFaceApiKey,
    ) -> Result<(), SecretStoreError>;

    fn delete_hugging_face_api_key(&self) -> Result<(), SecretStoreError>;
}

pub trait AsyncHuggingFaceApiKeyStore: Send + Sync {
    fn has_hugging_face_api_key_entry<'a>(&'a self) -> SecretStoreFuture<'a, bool>;

    fn read_hugging_face_api_key<'a>(&'a self) -> SecretStoreFuture<'a, Option<HuggingFaceApiKey>>;

    fn replace_hugging_face_api_key<'a>(
        &'a self,
        api_key: &'a HuggingFaceApiKey,
    ) -> SecretStoreFuture<'a, ()>;

    fn delete_hugging_face_api_key<'a>(&'a self) -> SecretStoreFuture<'a, ()>;
}

impl<S> AsyncHuggingFaceApiKeyStore for BlockingSecretStore<S>
where
    S: HuggingFaceApiKeyStore + Clone + Send + Sync + 'static,
{
    fn has_hugging_face_api_key_entry<'a>(&'a self) -> SecretStoreFuture<'a, bool> {
        let store = self.store.clone();
        Box::pin(async move {
            run_blocking_secret_operation(move || store.has_hugging_face_api_key_entry()).await
        })
    }

    fn read_hugging_face_api_key<'a>(&'a self) -> SecretStoreFuture<'a, Option<HuggingFaceApiKey>> {
        let store = self.store.clone();
        Box::pin(async move {
            run_blocking_secret_operation(move || store.read_hugging_face_api_key()).await
        })
    }

    fn replace_hugging_face_api_key<'a>(
        &'a self,
        api_key: &'a HuggingFaceApiKey,
    ) -> SecretStoreFuture<'a, ()> {
        let store = self.store.clone();
        let api_key = api_key.clone();
        Box::pin(async move {
            run_blocking_secret_operation(move || store.replace_hugging_face_api_key(&api_key))
                .await
        })
    }

    fn delete_hugging_face_api_key<'a>(&'a self) -> SecretStoreFuture<'a, ()> {
        let store = self.store.clone();
        Box::pin(async move {
            run_blocking_secret_operation(move || store.delete_hugging_face_api_key()).await
        })
    }
}

#[cfg(test)]
impl<S> AsyncHuggingFaceApiKeyStore for S
where
    S: HuggingFaceApiKeyStore + Send + Sync,
{
    fn has_hugging_face_api_key_entry<'a>(&'a self) -> SecretStoreFuture<'a, bool> {
        Box::pin(async move { HuggingFaceApiKeyStore::has_hugging_face_api_key_entry(self) })
    }

    fn read_hugging_face_api_key<'a>(&'a self) -> SecretStoreFuture<'a, Option<HuggingFaceApiKey>> {
        Box::pin(async move { HuggingFaceApiKeyStore::read_hugging_face_api_key(self) })
    }

    fn replace_hugging_face_api_key<'a>(
        &'a self,
        api_key: &'a HuggingFaceApiKey,
    ) -> SecretStoreFuture<'a, ()> {
        Box::pin(async move { HuggingFaceApiKeyStore::replace_hugging_face_api_key(self, api_key) })
    }

    fn delete_hugging_face_api_key<'a>(&'a self) -> SecretStoreFuture<'a, ()> {
        Box::pin(async move { HuggingFaceApiKeyStore::delete_hugging_face_api_key(self) })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    fn hugging_face_key(value: &str) -> HuggingFaceApiKey {
        HuggingFaceApiKey::new(value.to_string()).expect("hugging face key should be valid")
    }

    #[tokio::test]
    async fn blocking_hugging_face_key_store_delegates_operations_and_preserves_errors() {
        #[derive(Clone)]
        struct HuggingFaceOperationStore {
            result: Result<(), SecretStoreError>,
            calls: Arc<Mutex<Vec<&'static str>>>,
        }

        impl HuggingFaceApiKeyStore for HuggingFaceOperationStore {
            fn has_hugging_face_api_key_entry(&self) -> Result<bool, SecretStoreError> {
                self.calls.lock().expect("calls").push("has");
                self.result.clone()?;
                Ok(true)
            }

            fn read_hugging_face_api_key(
                &self,
            ) -> Result<Option<HuggingFaceApiKey>, SecretStoreError> {
                self.calls.lock().expect("calls").push("read");
                self.result.clone()?;
                Ok(Some(hugging_face_key("hf_secret")))
            }

            fn replace_hugging_face_api_key(
                &self,
                _api_key: &HuggingFaceApiKey,
            ) -> Result<(), SecretStoreError> {
                self.calls.lock().expect("calls").push("replace");
                self.result.clone()
            }

            fn delete_hugging_face_api_key(&self) -> Result<(), SecretStoreError> {
                self.calls.lock().expect("calls").push("delete");
                self.result.clone()
            }
        }

        let calls = Arc::new(Mutex::new(Vec::new()));
        let store = BlockingSecretStore::new(HuggingFaceOperationStore {
            result: Err(SecretStoreError::InvalidStoredHuggingFaceApiKey),
            calls: Arc::clone(&calls),
        });

        assert_eq!(
            AsyncHuggingFaceApiKeyStore::has_hugging_face_api_key_entry(&store).await,
            Err(SecretStoreError::InvalidStoredHuggingFaceApiKey)
        );
        assert_eq!(
            AsyncHuggingFaceApiKeyStore::read_hugging_face_api_key(&store)
                .await
                .map(|_| ()),
            Err(SecretStoreError::InvalidStoredHuggingFaceApiKey)
        );
        assert_eq!(
            AsyncHuggingFaceApiKeyStore::replace_hugging_face_api_key(
                &store,
                &hugging_face_key("hf_secret"),
            )
            .await,
            Err(SecretStoreError::InvalidStoredHuggingFaceApiKey)
        );
        assert_eq!(
            AsyncHuggingFaceApiKeyStore::delete_hugging_face_api_key(&store).await,
            Err(SecretStoreError::InvalidStoredHuggingFaceApiKey)
        );

        assert_eq!(
            calls.lock().expect("calls").as_slice(),
            ["has", "read", "replace", "delete"]
        );
    }
}
