use std::{future::Future, pin::Pin};

use thiserror::Error;

use crate::{
    domain::hugging_face_setup::{HuggingFaceApiKey, HuggingFaceApiKeySetup},
    provider::{huggingface::HuggingFaceClient, ProviderClientError},
    secrets::{AsyncHuggingFaceApiKeyStore, SecretStoreError},
};

pub type HuggingFaceSetupFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait HuggingFaceIdentityProvider: Send + Sync {
    fn validate_identity<'a>(
        &'a self,
        api_key: &'a HuggingFaceApiKey,
    ) -> HuggingFaceSetupFuture<'a, Result<HuggingFaceApiKeySetup, ProviderClientError>>;
}

impl HuggingFaceIdentityProvider for HuggingFaceClient {
    fn validate_identity<'a>(
        &'a self,
        api_key: &'a HuggingFaceApiKey,
    ) -> HuggingFaceSetupFuture<'a, Result<HuggingFaceApiKeySetup, ProviderClientError>> {
        Box::pin(async move { HuggingFaceClient::validate_identity(self, api_key).await })
    }
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum HuggingFaceSetupError {
    #[error("hugging face setup not found")]
    HuggingFaceSetupNotFound,
    #[error("hugging face api key is required")]
    HuggingFaceApiKeyRequired,
    #[error("hugging face api key unauthorized")]
    HuggingFaceApiKeyUnauthorized,
    #[error("hugging face api key has insufficient permissions")]
    HuggingFaceApiKeyInsufficientPermissions,
    #[error("stored hugging face api key invalid")]
    StoredHuggingFaceApiKeyInvalid,
    #[error("hugging face api unavailable")]
    HuggingFaceApiUnavailable,
    #[error("hugging face rate limited")]
    HuggingFaceRateLimited,
    #[error("hugging face identity response invalid")]
    HuggingFaceIdentityResponseInvalid,
    #[error("secure keyring unavailable")]
    SecureKeyringUnavailable,
}

impl From<SecretStoreError> for HuggingFaceSetupError {
    fn from(error: SecretStoreError) -> Self {
        match error {
            SecretStoreError::SecureKeyringUnavailable => Self::SecureKeyringUnavailable,
            SecretStoreError::InvalidStoredHuggingFaceApiKey => {
                Self::StoredHuggingFaceApiKeyInvalid
            }
            SecretStoreError::InvalidStoredProviderApiKey
            | SecretStoreError::InvalidStoredProvisionerWorkerToken => {
                Self::SecureKeyringUnavailable
            }
        }
    }
}

impl From<ProviderClientError> for HuggingFaceSetupError {
    fn from(error: ProviderClientError) -> Self {
        match error {
            ProviderClientError::Unauthorized => Self::HuggingFaceApiKeyUnauthorized,
            ProviderClientError::InsufficientPermissions => {
                Self::HuggingFaceApiKeyInsufficientPermissions
            }
            ProviderClientError::ApiUnavailable => Self::HuggingFaceApiUnavailable,
            ProviderClientError::RateLimited => Self::HuggingFaceRateLimited,
            ProviderClientError::ResponseInvalid => Self::HuggingFaceIdentityResponseInvalid,
            ProviderClientError::RequestRejected
            | ProviderClientError::NotFound
            | ProviderClientError::Conflict
            | ProviderClientError::Indeterminate => Self::HuggingFaceApiUnavailable,
        }
    }
}

pub struct HuggingFaceSetupService<S, P> {
    secrets: S,
    identity_provider: P,
}

impl<S, P> HuggingFaceSetupService<S, P> {
    pub fn new(secrets: S, identity_provider: P) -> Self {
        Self {
            secrets,
            identity_provider,
        }
    }
}

impl<S, P> HuggingFaceSetupService<S, P>
where
    S: AsyncHuggingFaceApiKeyStore,
    P: HuggingFaceIdentityProvider,
{
    pub async fn get_setup(&self) -> Result<Option<HuggingFaceApiKeySetup>, HuggingFaceSetupError> {
        let Some(api_key) = self.secrets.read_hugging_face_api_key().await? else {
            return Ok(None);
        };

        self.identity_provider
            .validate_identity(&api_key)
            .await
            .map(Some)
            .map_err(Into::into)
    }

    pub async fn setup(
        &self,
        api_key: HuggingFaceApiKey,
    ) -> Result<HuggingFaceApiKeySetup, HuggingFaceSetupError> {
        let setup = self.identity_provider.validate_identity(&api_key).await?;
        self.secrets.replace_hugging_face_api_key(&api_key).await?;

        Ok(setup)
    }

    pub async fn delete_setup(&self) -> Result<(), HuggingFaceSetupError> {
        if !self.secrets.has_hugging_face_api_key_entry().await? {
            return Err(HuggingFaceSetupError::HuggingFaceSetupNotFound);
        }

        self.secrets.delete_hugging_face_api_key().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::hugging_face_setup::{HuggingFaceApiKey, HuggingFaceApiKeySetup},
        provider::ProviderClientError,
        secrets::{HuggingFaceApiKeyStore, SecretStoreError},
    };
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Default)]
    struct FakeSecretStoreState {
        api_key: Option<String>,
        calls: Vec<&'static str>,
        replace_error: Option<SecretStoreError>,
    }

    #[derive(Debug, Clone, Default)]
    struct FakeSecretStore {
        state: Arc<Mutex<FakeSecretStoreState>>,
    }

    impl FakeSecretStore {
        fn with_api_key(api_key: &str) -> Self {
            let store = Self::default();
            store.state.lock().expect("fake store").api_key = Some(api_key.to_string());
            store
        }

        fn stored_key(&self) -> Option<String> {
            self.state.lock().expect("fake store").api_key.clone()
        }

        fn calls(&self) -> Vec<&'static str> {
            self.state.lock().expect("fake store").calls.clone()
        }

        fn fail_replace(&self, error: SecretStoreError) {
            self.state.lock().expect("fake store").replace_error = Some(error);
        }
    }

    impl HuggingFaceApiKeyStore for FakeSecretStore {
        fn has_hugging_face_api_key_entry(&self) -> Result<bool, SecretStoreError> {
            let mut state = self.state.lock().expect("fake store");
            state.calls.push("has");
            Ok(state.api_key.is_some())
        }

        fn read_hugging_face_api_key(&self) -> Result<Option<HuggingFaceApiKey>, SecretStoreError> {
            let mut state = self.state.lock().expect("fake store");
            state.calls.push("read");
            state
                .api_key
                .clone()
                .map(HuggingFaceApiKey::new)
                .transpose()
                .map_err(|_| SecretStoreError::InvalidStoredHuggingFaceApiKey)
        }

        fn replace_hugging_face_api_key(
            &self,
            api_key: &HuggingFaceApiKey,
        ) -> Result<(), SecretStoreError> {
            let mut state = self.state.lock().expect("fake store");
            state.calls.push("replace");
            if let Some(error) = state.replace_error.clone() {
                return Err(error);
            }
            state.api_key = Some(api_key.expose_secret().to_string());
            Ok(())
        }

        fn delete_hugging_face_api_key(&self) -> Result<(), SecretStoreError> {
            let mut state = self.state.lock().expect("fake store");
            state.calls.push("delete");
            state.api_key = None;
            Ok(())
        }
    }

    #[derive(Debug, Clone)]
    struct FakeIdentityProvider {
        result: Result<HuggingFaceApiKeySetup, ProviderClientError>,
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl FakeIdentityProvider {
        fn valid() -> Self {
            Self {
                result: Ok(setup()),
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn with_error(error: ProviderClientError) -> Self {
            Self {
                result: Err(error),
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().expect("provider calls").clone()
        }
    }

    impl HuggingFaceIdentityProvider for FakeIdentityProvider {
        fn validate_identity<'a>(
            &'a self,
            api_key: &'a HuggingFaceApiKey,
        ) -> HuggingFaceSetupFuture<'a, Result<HuggingFaceApiKeySetup, ProviderClientError>>
        {
            self.calls
                .lock()
                .expect("provider calls")
                .push(api_key.expose_secret().to_string());
            Box::pin(async move { self.result.clone() })
        }
    }

    fn key(value: &str) -> HuggingFaceApiKey {
        HuggingFaceApiKey::new(value.to_string()).expect("key should be valid")
    }

    fn setup() -> HuggingFaceApiKeySetup {
        HuggingFaceApiKeySetup {
            token_name: "RUNPOD_READ".to_string(),
            user_name: "pavel".to_string(),
            user_email: Some("pavel@example.com".to_string()),
        }
    }

    #[tokio::test]
    async fn get_setup_returns_none_when_key_is_missing() {
        let secrets = FakeSecretStore::default();
        let provider = FakeIdentityProvider::valid();
        let service = HuggingFaceSetupService::new(secrets.clone(), provider.clone());

        assert_eq!(service.get_setup().await, Ok(None));
        assert!(provider.calls().is_empty());
        assert_eq!(secrets.calls(), vec!["read"]);
    }

    #[tokio::test]
    async fn get_setup_validates_stored_key_before_returning_identity() {
        let secrets = FakeSecretStore::with_api_key("hf_stored");
        let provider = FakeIdentityProvider::valid();
        let service = HuggingFaceSetupService::new(secrets, provider.clone());

        assert_eq!(service.get_setup().await, Ok(Some(setup())));
        assert_eq!(provider.calls(), vec!["hf_stored"]);
    }

    #[tokio::test]
    async fn setup_validates_before_storing_key() {
        let secrets = FakeSecretStore::default();
        let provider = FakeIdentityProvider::valid();
        let service = HuggingFaceSetupService::new(secrets.clone(), provider.clone());

        assert_eq!(service.setup(key("hf_new")).await, Ok(setup()));
        assert_eq!(provider.calls(), vec!["hf_new"]);
        assert_eq!(secrets.calls(), vec!["replace"]);
        assert_eq!(secrets.stored_key(), Some("hf_new".to_string()));
    }

    #[tokio::test]
    async fn setup_does_not_store_unauthorized_key() {
        let secrets = FakeSecretStore::default();
        let provider = FakeIdentityProvider::with_error(ProviderClientError::Unauthorized);
        let service = HuggingFaceSetupService::new(secrets.clone(), provider);

        assert_eq!(
            service.setup(key("hf_bad")).await,
            Err(HuggingFaceSetupError::HuggingFaceApiKeyUnauthorized)
        );
        assert_eq!(secrets.calls(), Vec::<&'static str>::new());
        assert_eq!(secrets.stored_key(), None);
    }

    #[tokio::test]
    async fn setup_does_not_store_key_with_insufficient_permissions() {
        let secrets = FakeSecretStore::default();
        let provider =
            FakeIdentityProvider::with_error(ProviderClientError::InsufficientPermissions);
        let service = HuggingFaceSetupService::new(secrets.clone(), provider);

        assert_eq!(
            service.setup(key("hf_bad")).await,
            Err(HuggingFaceSetupError::HuggingFaceApiKeyInsufficientPermissions)
        );
        assert_eq!(secrets.calls(), Vec::<&'static str>::new());
        assert_eq!(secrets.stored_key(), None);
    }

    #[tokio::test]
    async fn setup_maps_keyring_write_failure() {
        let secrets = FakeSecretStore::default();
        secrets.fail_replace(SecretStoreError::SecureKeyringUnavailable);
        let provider = FakeIdentityProvider::valid();
        let service = HuggingFaceSetupService::new(secrets.clone(), provider);

        assert_eq!(
            service.setup(key("hf_new")).await,
            Err(HuggingFaceSetupError::SecureKeyringUnavailable)
        );
        assert_eq!(secrets.stored_key(), None);
    }

    #[tokio::test]
    async fn delete_setup_requires_existing_key() {
        let secrets = FakeSecretStore::default();
        let provider = FakeIdentityProvider::valid();
        let service = HuggingFaceSetupService::new(secrets.clone(), provider);

        assert_eq!(
            service.delete_setup().await,
            Err(HuggingFaceSetupError::HuggingFaceSetupNotFound)
        );
        assert_eq!(secrets.calls(), vec!["has"]);
    }
}
