use secrecy::SecretString;

use super::{
    Identity, SecretIdentityProvider, SecretIdentityProviderError, SecretKind, SecretStatus,
    SecretStore, SecretStoreError, SecretsError,
};

pub struct SecretsService<'a> {
    store: &'a dyn SecretStore,
    runpod_identity: &'a dyn SecretIdentityProvider,
    hugging_face_identity: &'a dyn SecretIdentityProvider,
}

impl<'a> SecretsService<'a> {
    pub fn new(
        store: &'a dyn SecretStore,
        runpod_identity: &'a dyn SecretIdentityProvider,
        hugging_face_identity: &'a dyn SecretIdentityProvider,
    ) -> Self {
        Self {
            store,
            runpod_identity,
            hugging_face_identity,
        }
    }

    #[crate::diagnostics::diagnostic(show_output, show_error)]
    pub async fn set(
        &self,
        #[diagnostic(show)] kind: SecretKind,
        #[diagnostic(redact)] secret: SecretString,
    ) -> Result<Identity, SecretsError> {
        if self
            .store
            .exists(kind)
            .await
            .map_err(|_| SecretsError::StorageUnavailable)?
        {
            return Err(SecretsError::AlreadyConfigured);
        }

        let identity = self
            .provider(kind)
            .identity(&secret)
            .await
            .map_err(map_identity_error)?;
        self.store
            .insert(kind, secret)
            .await
            .map_err(|error| match error {
                SecretStoreError::AlreadyExists => SecretsError::AlreadyConfigured,
                SecretStoreError::NotFound | SecretStoreError::Unavailable => {
                    SecretsError::StorageUnavailable
                }
            })?;
        Ok(identity)
    }

    #[crate::diagnostics::diagnostic(show_output, show_error)]
    pub async fn status(
        &self,
        #[diagnostic(show)] kind: SecretKind,
    ) -> Result<SecretStatus, SecretsError> {
        self.store
            .exists(kind)
            .await
            .map(|exists| {
                if exists {
                    SecretStatus::Configured
                } else {
                    SecretStatus::Missing
                }
            })
            .map_err(|_| SecretsError::StorageUnavailable)
    }

    #[crate::diagnostics::diagnostic(show_output, show_error)]
    pub async fn identity(
        &self,
        #[diagnostic(show)] kind: SecretKind,
    ) -> Result<Identity, SecretsError> {
        let secret = self
            .store
            .get(kind)
            .await
            .map_err(|_| SecretsError::StorageUnavailable)?
            .ok_or(SecretsError::NotConfigured)?;
        self.provider(kind)
            .identity(&secret)
            .await
            .map_err(map_identity_error)
    }

    #[crate::diagnostics::diagnostic(show_error)]
    pub async fn delete(&self, #[diagnostic(show)] kind: SecretKind) -> Result<(), SecretsError> {
        self.store.delete(kind).await.map_err(|error| match error {
            SecretStoreError::NotFound => SecretsError::NotConfigured,
            SecretStoreError::AlreadyExists | SecretStoreError::Unavailable => {
                SecretsError::StorageUnavailable
            }
        })
    }

    fn provider(&self, kind: SecretKind) -> &dyn SecretIdentityProvider {
        match kind {
            SecretKind::RunpodApiKey => self.runpod_identity,
            SecretKind::HuggingFaceApiKey => self.hugging_face_identity,
        }
    }
}

fn map_identity_error(error: SecretIdentityProviderError) -> SecretsError {
    match error {
        SecretIdentityProviderError::InvalidCredential => SecretsError::InvalidCredential,
        SecretIdentityProviderError::Unavailable => SecretsError::IdentityUnavailable,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use secrecy::SecretString;

    use super::super::{
        Identity, SecretIdentityProvider, SecretIdentityProviderError, SecretKind, SecretStatus,
        SecretStore, SecretStoreError, SecretsError, SecretsService,
    };

    struct FakeStore {
        configured: Mutex<Vec<SecretKind>>,
        calls: Arc<Mutex<Vec<&'static str>>>,
    }

    #[async_trait::async_trait]
    impl SecretStore for FakeStore {
        async fn exists(&self, kind: SecretKind) -> Result<bool, SecretStoreError> {
            self.calls.lock().unwrap().push(call("exists", kind));
            Ok(self.configured.lock().unwrap().contains(&kind))
        }

        async fn get(&self, kind: SecretKind) -> Result<Option<SecretString>, SecretStoreError> {
            self.calls.lock().unwrap().push(call("get", kind));
            Ok(self
                .configured
                .lock()
                .unwrap()
                .contains(&kind)
                .then(|| SecretString::from("stored")))
        }

        async fn insert(
            &self,
            kind: SecretKind,
            _secret: SecretString,
        ) -> Result<(), SecretStoreError> {
            self.calls.lock().unwrap().push(call("insert", kind));
            self.configured.lock().unwrap().push(kind);
            Ok(())
        }

        async fn delete(&self, kind: SecretKind) -> Result<(), SecretStoreError> {
            let mut configured = self.configured.lock().unwrap();
            configured
                .iter()
                .position(|candidate| *candidate == kind)
                .map(|index| configured.remove(index))
                .ok_or(SecretStoreError::NotFound)?;
            Ok(())
        }
    }

    struct FakeIdentityProvider {
        kind: SecretKind,
        calls: Arc<Mutex<Vec<&'static str>>>,
    }

    #[async_trait::async_trait]
    impl SecretIdentityProvider for FakeIdentityProvider {
        async fn identity(
            &self,
            _credential: &SecretString,
        ) -> Result<Identity, SecretIdentityProviderError> {
            self.calls.lock().unwrap().push(call("identity", self.kind));
            Ok(Identity {
                key_name: None,
                username: None,
                email: Some("user@example.com".to_owned()),
            })
        }
    }

    struct Fakes {
        store: FakeStore,
        runpod: FakeIdentityProvider,
        hugging_face: FakeIdentityProvider,
        calls: Arc<Mutex<Vec<&'static str>>>,
    }

    impl Fakes {
        fn empty() -> Self {
            Self::new(Vec::new())
        }

        fn configured(kind: SecretKind) -> Self {
            Self::new(vec![kind])
        }

        fn new(configured: Vec<SecretKind>) -> Self {
            let calls = Arc::new(Mutex::new(Vec::new()));
            Self {
                store: FakeStore {
                    configured: Mutex::new(configured),
                    calls: calls.clone(),
                },
                runpod: FakeIdentityProvider {
                    kind: SecretKind::RunpodApiKey,
                    calls: calls.clone(),
                },
                hugging_face: FakeIdentityProvider {
                    kind: SecretKind::HuggingFaceApiKey,
                    calls: calls.clone(),
                },
                calls,
            }
        }

        fn service(&self) -> SecretsService<'_> {
            SecretsService::new(&self.store, &self.runpod, &self.hugging_face)
        }

        fn calls(&self) -> Vec<&'static str> {
            self.calls.lock().unwrap().clone()
        }
    }

    fn call(operation: &str, kind: SecretKind) -> &'static str {
        match (operation, kind) {
            ("exists", SecretKind::RunpodApiKey) => "exists:runpod",
            ("exists", SecretKind::HuggingFaceApiKey) => "exists:hugging_face",
            ("get", SecretKind::RunpodApiKey) => "get:runpod",
            ("get", SecretKind::HuggingFaceApiKey) => "get:hugging_face",
            ("insert", SecretKind::RunpodApiKey) => "insert:runpod",
            ("insert", SecretKind::HuggingFaceApiKey) => "insert:hugging_face",
            ("identity", SecretKind::RunpodApiKey) => "identity:runpod",
            ("identity", SecretKind::HuggingFaceApiKey) => "identity:hugging_face",
            _ => unreachable!(),
        }
    }

    #[tokio::test]
    async fn set_validates_before_inserting() {
        let fakes = Fakes::empty();
        let identity = fakes
            .service()
            .set(SecretKind::RunpodApiKey, SecretString::from("candidate"))
            .await
            .unwrap();

        assert_eq!(identity.email.as_deref(), Some("user@example.com"));
        assert_eq!(
            fakes.calls(),
            vec!["exists:runpod", "identity:runpod", "insert:runpod"]
        );
    }

    #[tokio::test]
    async fn set_rejects_an_existing_key_without_network_validation() {
        let fakes = Fakes::configured(SecretKind::RunpodApiKey);

        assert_eq!(
            fakes
                .service()
                .set(SecretKind::RunpodApiKey, SecretString::from("candidate"))
                .await,
            Err(SecretsError::AlreadyConfigured)
        );
        assert_eq!(fakes.calls(), vec!["exists:runpod"]);
    }

    #[tokio::test]
    async fn delete_missing_key_is_an_explicit_error() {
        let fakes = Fakes::empty();

        assert_eq!(
            fakes.service().delete(SecretKind::HuggingFaceApiKey).await,
            Err(SecretsError::NotConfigured)
        );
    }

    #[tokio::test]
    async fn status_does_not_read_the_raw_secret_or_call_the_network() {
        let fakes = Fakes::configured(SecretKind::RunpodApiKey);

        assert_eq!(
            fakes
                .service()
                .status(SecretKind::RunpodApiKey)
                .await
                .unwrap(),
            SecretStatus::Configured
        );
        assert_eq!(fakes.calls(), vec!["exists:runpod"]);
    }
}
