mod coordinator;
mod error;
mod providers;

pub use coordinator::ProviderSetupCoordinator;
pub use error::ProviderSetupError;

use crate::{
    domain::provider_setup::{
        self, GpuCloudProviderId as DomainGpuCloudProviderId,
        GpuCloudProviderSetup as DomainGpuCloudProviderSetup, ProviderApiKey, ProviderIdentity,
    },
    secrets::AsyncSecretStore,
};

pub use providers::{
    ProviderSetupCapability, ProviderSetupProviderRegistry, ProviderSetupProviderResolver,
};

pub struct ProviderSetupService<S, R = ProviderSetupProviderRegistry> {
    secrets: S,
    provider_registry: R,
}

impl<S> ProviderSetupService<S> {
    pub fn new(secrets: S) -> Self {
        Self::with_provider_registry(secrets, ProviderSetupProviderRegistry::default())
    }
}

impl<S, R> ProviderSetupService<S, R> {
    pub fn with_provider_registry(secrets: S, provider_registry: R) -> Self {
        Self {
            secrets,
            provider_registry,
        }
    }
}

impl<S, R> ProviderSetupService<S, R>
where
    S: AsyncSecretStore,
    R: ProviderSetupProviderResolver,
{
    pub async fn get_setup(
        &self,
        provider_id: DomainGpuCloudProviderId,
    ) -> Result<Option<DomainGpuCloudProviderSetup>, ProviderSetupError> {
        let Some(api_key) = self.secrets.read_api_key(&provider_id).await? else {
            return Ok(None);
        };

        let setup = self.setup_from_key(&provider_id, &api_key).await?;

        Ok(Some(setup))
    }

    pub async fn setup(
        &self,
        provider_id: DomainGpuCloudProviderId,
        api_key: ProviderApiKey,
    ) -> Result<DomainGpuCloudProviderSetup, ProviderSetupError> {
        if self.secrets.read_api_key(&provider_id).await?.is_some() {
            return Err(ProviderSetupError::ProviderSetupAlreadyExists);
        }

        self.provider_registry
            .for_provider(&provider_id)
            .validate_identity(&api_key)
            .await?;
        self.secrets.replace_api_key(&provider_id, &api_key).await?;

        let setup = match self.finalize_setup_from_stored_key(&provider_id).await {
            Ok(setup) => setup,
            Err(error) => return Err(self.rollback_failed_setup(&provider_id, error).await),
        };

        Ok(setup)
    }

    pub async fn delete_setup(
        &self,
        provider_id: DomainGpuCloudProviderId,
    ) -> Result<(), ProviderSetupError> {
        if !self.secrets.has_api_key_entry(&provider_id).await? {
            return Err(ProviderSetupError::ProviderSetupNotFound);
        }

        self.secrets.delete_api_key(&provider_id).await?;

        Ok(())
    }

    async fn finalize_setup_from_stored_key(
        &self,
        provider_id: &DomainGpuCloudProviderId,
    ) -> Result<DomainGpuCloudProviderSetup, ProviderSetupError> {
        let stored_api_key = self
            .secrets
            .read_api_key(provider_id)
            .await?
            .ok_or(ProviderSetupError::SecureKeyringUnavailable)?;

        self.setup_from_key(provider_id, &stored_api_key).await
    }

    async fn rollback_failed_setup(
        &self,
        provider_id: &DomainGpuCloudProviderId,
        finalization_error: ProviderSetupError,
    ) -> ProviderSetupError {
        match self.secrets.delete_api_key(provider_id).await {
            Ok(()) => finalization_error,
            Err(_) => ProviderSetupError::ProviderSetupRecoveryRequired,
        }
    }

    async fn setup_from_key(
        &self,
        provider_id: &DomainGpuCloudProviderId,
        api_key: &ProviderApiKey,
    ) -> Result<DomainGpuCloudProviderSetup, ProviderSetupError> {
        let identity = self
            .provider_registry
            .for_provider(provider_id)
            .validate_identity(api_key)
            .await?;
        provider_setup::validator::validate_provider_identity(&identity)
            .map_err(|_| ProviderSetupError::ProviderIdentityResponseInvalid)?;
        let setup = Self::setup_from_identity(*provider_id, identity);
        provider_setup::validator::validate_gpu_cloud_provider_setup(&setup)
            .map_err(|_| ProviderSetupError::ProviderIdentityResponseInvalid)?;

        Ok(setup)
    }

    fn setup_from_identity(
        provider_id: DomainGpuCloudProviderId,
        identity: ProviderIdentity,
    ) -> DomainGpuCloudProviderSetup {
        DomainGpuCloudProviderSetup {
            gpu_cloud_provider_id: provider_id,
            provider_user_email: identity.provider_user_email,
            provider_api_key_fingerprint: identity.provider_api_key_fingerprint,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::{ProvisionerWorkerBearerToken, SecretStore, SecretStoreError};
    use std::{
        collections::VecDeque,
        future::Future,
        pin::Pin,
        sync::{Arc, Mutex},
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum SecretStoreCall {
        HasApiKeyEntry,
        ReadApiKey,
        ReplaceApiKey(String),
        DeleteApiKey,
    }

    #[derive(Debug, Default)]
    struct FakeSecretStoreState {
        api_key: Option<String>,
        calls: Vec<SecretStoreCall>,
        read_calls: usize,
        fail_reads: VecDeque<(usize, SecretStoreError)>,
        has_entry_error: Option<SecretStoreError>,
        replace_error: Option<SecretStoreError>,
        delete_error: Option<SecretStoreError>,
    }

    #[derive(Debug, Clone, Default)]
    struct FakeSecretStore {
        state: Arc<Mutex<FakeSecretStoreState>>,
    }

    impl FakeSecretStore {
        fn with_api_key(api_key: impl Into<String>) -> Self {
            let store = Self::default();
            store.state.lock().expect("fake store mutex").api_key = Some(api_key.into());
            store
        }

        fn fail_read(&self, read_call: usize, error: SecretStoreError) {
            self.state
                .lock()
                .expect("fake store mutex")
                .fail_reads
                .push_back((read_call, error));
        }

        fn set_replace_error(&self, error: SecretStoreError) {
            self.state.lock().expect("fake store mutex").replace_error = Some(error);
        }

        fn set_delete_error(&self, error: SecretStoreError) {
            self.state.lock().expect("fake store mutex").delete_error = Some(error);
        }

        fn calls(&self) -> Vec<SecretStoreCall> {
            self.state.lock().expect("fake store mutex").calls.clone()
        }

        fn stored_key(&self) -> Option<String> {
            self.state.lock().expect("fake store mutex").api_key.clone()
        }
    }

    impl SecretStore for FakeSecretStore {
        fn has_api_key_entry(
            &self,
            _provider_id: &DomainGpuCloudProviderId,
        ) -> Result<bool, SecretStoreError> {
            let mut state = self.state.lock().expect("fake store mutex");
            state.calls.push(SecretStoreCall::HasApiKeyEntry);
            if let Some(error) = state.has_entry_error.clone() {
                return Err(error);
            }

            Ok(state.api_key.is_some())
        }

        fn read_api_key(
            &self,
            _provider_id: &DomainGpuCloudProviderId,
        ) -> Result<Option<ProviderApiKey>, SecretStoreError> {
            let mut state = self.state.lock().expect("fake store mutex");
            state.calls.push(SecretStoreCall::ReadApiKey);
            state.read_calls += 1;
            let read_calls = state.read_calls;
            if state
                .fail_reads
                .front()
                .is_some_and(|(call, _)| *call == read_calls)
            {
                let (_, error) = state.fail_reads.pop_front().expect("front exists");
                return Err(error);
            }

            state
                .api_key
                .clone()
                .map(ProviderApiKey::new)
                .transpose()
                .map_err(|_| SecretStoreError::InvalidStoredProviderApiKey)
        }

        fn replace_api_key(
            &self,
            _provider_id: &DomainGpuCloudProviderId,
            api_key: &ProviderApiKey,
        ) -> Result<(), SecretStoreError> {
            let mut state = self.state.lock().expect("fake store mutex");
            state.calls.push(SecretStoreCall::ReplaceApiKey(
                api_key.expose_secret().to_string(),
            ));
            if let Some(error) = state.replace_error.clone() {
                return Err(error);
            }

            state.api_key = Some(api_key.expose_secret().to_string());
            Ok(())
        }

        fn delete_api_key(
            &self,
            _provider_id: &DomainGpuCloudProviderId,
        ) -> Result<(), SecretStoreError> {
            let mut state = self.state.lock().expect("fake store mutex");
            state.calls.push(SecretStoreCall::DeleteApiKey);
            if let Some(error) = state.delete_error.clone() {
                return Err(error);
            }

            state.api_key = None;
            Ok(())
        }

        fn write_provisioner_worker_token(
            &self,
            _workspace_id: &str,
            _token: &ProvisionerWorkerBearerToken,
        ) -> Result<(), SecretStoreError> {
            Ok(())
        }

        fn read_provisioner_worker_token(
            &self,
            _workspace_id: &str,
        ) -> Result<Option<ProvisionerWorkerBearerToken>, SecretStoreError> {
            Ok(None)
        }

        fn delete_provisioner_worker_token(
            &self,
            _workspace_id: &str,
        ) -> Result<(), SecretStoreError> {
            Ok(())
        }
    }

    #[derive(Debug, Default)]
    struct FakeProviderSetupCapability {
        results: Mutex<VecDeque<Result<ProviderIdentity, ProviderSetupError>>>,
        calls: Mutex<Vec<String>>,
    }

    impl FakeProviderSetupCapability {
        fn calls(&self) -> Vec<String> {
            self.calls.lock().expect("fake capability mutex").clone()
        }
    }

    impl ProviderSetupCapability for FakeProviderSetupCapability {
        fn validate_identity<'a>(
            &'a self,
            api_key: &'a ProviderApiKey,
        ) -> Pin<Box<dyn Future<Output = Result<ProviderIdentity, ProviderSetupError>> + Send + 'a>>
        {
            self.calls
                .lock()
                .expect("fake capability mutex")
                .push(api_key.expose_secret().to_string());
            let result = self
                .results
                .lock()
                .expect("fake capability mutex")
                .pop_front()
                .expect("fake capability result");

            Box::pin(async move { result })
        }
    }

    #[derive(Debug)]
    struct FakeProviderSetupRegistry {
        capability: FakeProviderSetupCapability,
        provider_calls: Mutex<Vec<DomainGpuCloudProviderId>>,
    }

    impl FakeProviderSetupRegistry {
        fn with_results(
            results: impl IntoIterator<Item = Result<ProviderIdentity, ProviderSetupError>>,
        ) -> Arc<Self> {
            Arc::new(Self {
                capability: FakeProviderSetupCapability {
                    results: Mutex::new(results.into_iter().collect()),
                    calls: Mutex::new(Vec::new()),
                },
                provider_calls: Mutex::new(Vec::new()),
            })
        }

        fn identity_calls(&self) -> Vec<String> {
            self.capability.calls()
        }

        fn provider_calls(&self) -> Vec<DomainGpuCloudProviderId> {
            self.provider_calls
                .lock()
                .expect("fake registry mutex")
                .clone()
        }
    }

    impl ProviderSetupProviderResolver for Arc<FakeProviderSetupRegistry> {
        fn for_provider(
            &self,
            provider_id: &DomainGpuCloudProviderId,
        ) -> &dyn ProviderSetupCapability {
            self.provider_calls
                .lock()
                .expect("fake registry mutex")
                .push(*provider_id);
            &self.capability
        }
    }

    fn valid_identity() -> ProviderIdentity {
        ProviderIdentity {
            provider_user_email: "user@example.com".to_string(),
            provider_api_key_fingerprint: "rp_key".to_string(),
        }
    }

    fn api_key(value: &str) -> ProviderApiKey {
        ProviderApiKey::new(value.to_string()).expect("test key should be valid")
    }

    fn service(
        secrets: FakeSecretStore,
        registry: Arc<FakeProviderSetupRegistry>,
    ) -> ProviderSetupService<FakeSecretStore, Arc<FakeProviderSetupRegistry>> {
        ProviderSetupService::with_provider_registry(secrets, registry)
    }

    #[tokio::test]
    async fn get_setup_returns_none_when_no_key_is_stored() {
        let secrets = FakeSecretStore::default();
        let registry = FakeProviderSetupRegistry::with_results([]);

        let result = service(secrets.clone(), registry.clone())
            .get_setup(DomainGpuCloudProviderId::Runpod)
            .await;

        assert!(result.expect("get setup should succeed").is_none());
        assert_eq!(secrets.calls(), vec![SecretStoreCall::ReadApiKey]);
        assert!(registry.identity_calls().is_empty());
        assert!(registry.provider_calls().is_empty());
    }

    #[tokio::test]
    async fn get_setup_derives_redacted_state_from_stored_key() {
        let secrets = FakeSecretStore::with_api_key("rp_key_secret");
        let registry = FakeProviderSetupRegistry::with_results([Ok(valid_identity())]);

        let setup = service(secrets.clone(), registry.clone())
            .get_setup(DomainGpuCloudProviderId::Runpod)
            .await
            .expect("get setup should succeed")
            .expect("setup should exist");

        assert_eq!(
            setup.gpu_cloud_provider_id,
            DomainGpuCloudProviderId::Runpod
        );
        assert_eq!(setup.provider_user_email, "user@example.com");
        assert_eq!(setup.provider_api_key_fingerprint, "rp_key");
        assert_eq!(registry.identity_calls(), vec!["rp_key_secret".to_string()]);
        assert_eq!(
            registry.provider_calls(),
            vec![DomainGpuCloudProviderId::Runpod]
        );
    }

    #[tokio::test]
    async fn get_setup_reports_malformed_stored_key() {
        let secrets = FakeSecretStore::with_api_key(" \t");
        let registry = FakeProviderSetupRegistry::with_results([]);

        let result = service(secrets, registry)
            .get_setup(DomainGpuCloudProviderId::Runpod)
            .await;

        assert!(matches!(
            result,
            Err(ProviderSetupError::StoredProviderApiKeyInvalid)
        ));
    }

    #[tokio::test]
    async fn setup_rejects_existing_setup_before_provider_validation_or_mutation() {
        let secrets = FakeSecretStore::with_api_key("rp_existing_secret");
        let registry = FakeProviderSetupRegistry::with_results([]);

        let result = service(secrets.clone(), registry.clone())
            .setup(DomainGpuCloudProviderId::Runpod, api_key("rp_new_secret"))
            .await;

        assert!(matches!(
            result,
            Err(ProviderSetupError::ProviderSetupAlreadyExists)
        ));
        assert!(registry.identity_calls().is_empty());
        assert!(registry.provider_calls().is_empty());
        assert_eq!(secrets.stored_key(), Some("rp_existing_secret".to_string()));
        assert_eq!(secrets.calls(), vec![SecretStoreCall::ReadApiKey]);
    }

    #[tokio::test]
    async fn setup_rejects_unauthorized_submitted_key_without_mutating_keyring() {
        let secrets = FakeSecretStore::default();
        let registry = FakeProviderSetupRegistry::with_results([Err(
            ProviderSetupError::ProviderApiKeyUnauthorized,
        )]);

        let result = service(secrets.clone(), registry.clone())
            .setup(DomainGpuCloudProviderId::Runpod, api_key("rp_bad_secret"))
            .await;

        assert!(matches!(
            result,
            Err(ProviderSetupError::ProviderApiKeyUnauthorized)
        ));
        assert_eq!(registry.identity_calls(), vec!["rp_bad_secret".to_string()]);
        assert_eq!(
            registry.provider_calls(),
            vec![DomainGpuCloudProviderId::Runpod]
        );
        assert_eq!(secrets.stored_key(), None);
        assert_eq!(secrets.calls(), vec![SecretStoreCall::ReadApiKey]);
    }

    #[tokio::test]
    async fn setup_validates_writes_rereads_and_derives_setup_from_stored_key() {
        let secrets = FakeSecretStore::default();
        let registry = FakeProviderSetupRegistry::with_results([
            Ok(ProviderIdentity {
                provider_user_email: "submitted@example.com".to_string(),
                provider_api_key_fingerprint: "submitted-key".to_string(),
            }),
            Ok(valid_identity()),
        ]);

        let setup = service(secrets.clone(), registry.clone())
            .setup(DomainGpuCloudProviderId::Runpod, api_key("rp_key_secret"))
            .await
            .expect("setup should succeed");

        assert_eq!(setup.provider_user_email, "user@example.com");
        assert_eq!(setup.provider_api_key_fingerprint, "rp_key");
        assert_eq!(
            secrets.calls(),
            vec![
                SecretStoreCall::ReadApiKey,
                SecretStoreCall::ReplaceApiKey("rp_key_secret".to_string()),
                SecretStoreCall::ReadApiKey,
            ]
        );
        assert_eq!(
            registry.identity_calls(),
            vec!["rp_key_secret".to_string(), "rp_key_secret".to_string()]
        );
        assert_eq!(
            registry.provider_calls(),
            vec![
                DomainGpuCloudProviderId::Runpod,
                DomainGpuCloudProviderId::Runpod
            ]
        );
    }

    #[tokio::test]
    async fn setup_reports_keyring_write_failure_without_success() {
        let secrets = FakeSecretStore::default();
        secrets.set_replace_error(SecretStoreError::SecureKeyringUnavailable);
        let registry = FakeProviderSetupRegistry::with_results([Ok(valid_identity())]);

        let result = service(secrets.clone(), registry)
            .setup(DomainGpuCloudProviderId::Runpod, api_key("rp_key_secret"))
            .await;

        assert!(matches!(
            result,
            Err(ProviderSetupError::SecureKeyringUnavailable)
        ));
        assert_eq!(secrets.stored_key(), None);
    }

    #[tokio::test]
    async fn setup_rolls_back_when_stored_key_reread_fails() {
        let secrets = FakeSecretStore::default();
        secrets.fail_read(2, SecretStoreError::SecureKeyringUnavailable);
        let registry = FakeProviderSetupRegistry::with_results([Ok(valid_identity())]);

        let result = service(secrets.clone(), registry)
            .setup(DomainGpuCloudProviderId::Runpod, api_key("rp_key_secret"))
            .await;

        assert!(matches!(
            result,
            Err(ProviderSetupError::SecureKeyringUnavailable)
        ));
        assert_eq!(secrets.stored_key(), None);
        assert_eq!(
            secrets.calls(),
            vec![
                SecretStoreCall::ReadApiKey,
                SecretStoreCall::ReplaceApiKey("rp_key_secret".to_string()),
                SecretStoreCall::ReadApiKey,
                SecretStoreCall::DeleteApiKey,
            ]
        );
    }

    #[tokio::test]
    async fn setup_rolls_back_when_stored_key_validation_fails() {
        let secrets = FakeSecretStore::default();
        let registry = FakeProviderSetupRegistry::with_results([
            Ok(valid_identity()),
            Err(ProviderSetupError::ProviderApiUnavailable),
        ]);

        let result = service(secrets.clone(), registry)
            .setup(DomainGpuCloudProviderId::Runpod, api_key("rp_key_secret"))
            .await;

        assert!(matches!(
            result,
            Err(ProviderSetupError::ProviderApiUnavailable)
        ));
        assert_eq!(secrets.stored_key(), None);
    }

    #[tokio::test]
    async fn setup_reports_recovery_required_when_rollback_fails() {
        let secrets = FakeSecretStore::default();
        secrets.set_delete_error(SecretStoreError::SecureKeyringUnavailable);
        let registry = FakeProviderSetupRegistry::with_results([
            Ok(valid_identity()),
            Err(ProviderSetupError::ProviderApiUnavailable),
        ]);

        let result = service(secrets.clone(), registry)
            .setup(DomainGpuCloudProviderId::Runpod, api_key("rp_key_secret"))
            .await;

        assert!(matches!(
            result,
            Err(ProviderSetupError::ProviderSetupRecoveryRequired)
        ));
        assert_eq!(secrets.stored_key(), Some("rp_key_secret".to_string()));
    }

    #[tokio::test]
    async fn delete_setup_removes_existing_or_corrupt_local_entry_without_reading_provider_key() {
        let secrets = FakeSecretStore::with_api_key(" \t");
        let registry = FakeProviderSetupRegistry::with_results([]);

        service(secrets.clone(), registry)
            .delete_setup(DomainGpuCloudProviderId::Runpod)
            .await
            .expect("delete should succeed");

        assert_eq!(secrets.stored_key(), None);
        assert_eq!(
            secrets.calls(),
            vec![
                SecretStoreCall::HasApiKeyEntry,
                SecretStoreCall::DeleteApiKey
            ]
        );
    }

    #[tokio::test]
    async fn delete_setup_reports_missing_setup() {
        let secrets = FakeSecretStore::default();
        let registry = FakeProviderSetupRegistry::with_results([]);

        let result = service(secrets.clone(), registry)
            .delete_setup(DomainGpuCloudProviderId::Runpod)
            .await;

        assert_eq!(result, Err(ProviderSetupError::ProviderSetupNotFound));
        assert_eq!(secrets.calls(), vec![SecretStoreCall::HasApiKeyEntry]);
    }
}
