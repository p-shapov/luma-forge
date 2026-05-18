mod coordinator;
mod error;
mod providers;

use std::{future::Future, pin::Pin};

pub use coordinator::ProviderSetupCoordinator;
pub use error::ProviderSetupError;

use crate::{
    domain::provider_setup::{
        self, GpuCloudProviderId as DomainGpuCloudProviderId,
        GpuCloudProviderSetup as DomainGpuCloudProviderSetup, ProviderApiKey, ProviderIdentity,
    },
    secrets::SecretStore,
};

pub trait ProviderIdentityValidator: Send + Sync {
    fn validate_identity<'a>(
        &'a self,
        provider_id: &'a DomainGpuCloudProviderId,
        api_key: &'a ProviderApiKey,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderIdentity, ProviderSetupError>> + Send + 'a>>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ProductionProviderIdentityValidator;

impl ProviderIdentityValidator for ProductionProviderIdentityValidator {
    fn validate_identity<'a>(
        &'a self,
        provider_id: &'a DomainGpuCloudProviderId,
        api_key: &'a ProviderApiKey,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderIdentity, ProviderSetupError>> + Send + 'a>>
    {
        Box::pin(providers::validate_identity(provider_id, api_key))
    }
}

pub struct ProviderSetupService<S, V = ProductionProviderIdentityValidator> {
    secrets: S,
    identity_validator: V,
}

impl<S> ProviderSetupService<S> {
    pub fn new(secrets: S) -> Self {
        Self::with_identity_validator(secrets, ProductionProviderIdentityValidator)
    }
}

impl<S, V> ProviderSetupService<S, V> {
    pub fn with_identity_validator(secrets: S, identity_validator: V) -> Self {
        Self {
            secrets,
            identity_validator,
        }
    }
}

impl<S, V> ProviderSetupService<S, V>
where
    S: SecretStore,
    V: ProviderIdentityValidator,
{
    pub async fn get_setup(
        &self,
        provider_id: DomainGpuCloudProviderId,
    ) -> Result<Option<DomainGpuCloudProviderSetup>, ProviderSetupError> {
        let Some(api_key) = self.secrets.read_api_key(&provider_id)? else {
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
        if self.secrets.read_api_key(&provider_id)?.is_some() {
            return Err(ProviderSetupError::ProviderSetupAlreadyExists);
        }

        self.identity_validator
            .validate_identity(&provider_id, &api_key)
            .await?;
        self.secrets.replace_api_key(&provider_id, &api_key)?;

        let setup = match self.finalize_setup_from_stored_key(&provider_id).await {
            Ok(setup) => setup,
            Err(error) => return Err(self.rollback_failed_setup(&provider_id, error)),
        };

        Ok(setup)
    }

    pub fn delete_setup(
        &self,
        provider_id: DomainGpuCloudProviderId,
    ) -> Result<(), ProviderSetupError> {
        if !self.secrets.has_api_key_entry(&provider_id)? {
            return Err(ProviderSetupError::ProviderSetupNotFound);
        }

        self.secrets.delete_api_key(&provider_id)?;

        Ok(())
    }

    async fn finalize_setup_from_stored_key(
        &self,
        provider_id: &DomainGpuCloudProviderId,
    ) -> Result<DomainGpuCloudProviderSetup, ProviderSetupError> {
        let stored_api_key = self
            .secrets
            .read_api_key(provider_id)?
            .ok_or(ProviderSetupError::SecureKeyringUnavailable)?;

        self.setup_from_key(provider_id, &stored_api_key).await
    }

    fn rollback_failed_setup(
        &self,
        provider_id: &DomainGpuCloudProviderId,
        finalization_error: ProviderSetupError,
    ) -> ProviderSetupError {
        match self.secrets.delete_api_key(provider_id) {
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
            .identity_validator
            .validate_identity(provider_id, api_key)
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
    use crate::secrets::{ProvisionerWorkerBearerToken, SecretStoreError};
    use std::{
        collections::VecDeque,
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
    struct FakeIdentityValidator {
        results: Mutex<VecDeque<Result<ProviderIdentity, ProviderSetupError>>>,
        calls: Mutex<Vec<String>>,
    }

    impl FakeIdentityValidator {
        fn with_results(
            results: impl IntoIterator<Item = Result<ProviderIdentity, ProviderSetupError>>,
        ) -> Arc<Self> {
            Arc::new(Self {
                results: Mutex::new(results.into_iter().collect()),
                calls: Mutex::new(Vec::new()),
            })
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().expect("fake validator mutex").clone()
        }
    }

    impl ProviderIdentityValidator for Arc<FakeIdentityValidator> {
        fn validate_identity<'a>(
            &'a self,
            _provider_id: &'a DomainGpuCloudProviderId,
            api_key: &'a ProviderApiKey,
        ) -> Pin<Box<dyn Future<Output = Result<ProviderIdentity, ProviderSetupError>> + Send + 'a>>
        {
            self.calls
                .lock()
                .expect("fake validator mutex")
                .push(api_key.expose_secret().to_string());
            let result = self
                .results
                .lock()
                .expect("fake validator mutex")
                .pop_front()
                .expect("fake validator result");

            Box::pin(async move { result })
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
        validator: Arc<FakeIdentityValidator>,
    ) -> ProviderSetupService<FakeSecretStore, Arc<FakeIdentityValidator>> {
        ProviderSetupService::with_identity_validator(secrets, validator)
    }

    #[tokio::test]
    async fn get_setup_returns_none_when_no_key_is_stored() {
        let secrets = FakeSecretStore::default();
        let validator = FakeIdentityValidator::with_results([]);

        let result = service(secrets.clone(), validator.clone())
            .get_setup(DomainGpuCloudProviderId::Runpod)
            .await;

        assert!(result.expect("get setup should succeed").is_none());
        assert_eq!(secrets.calls(), vec![SecretStoreCall::ReadApiKey]);
        assert!(validator.calls().is_empty());
    }

    #[tokio::test]
    async fn get_setup_derives_redacted_state_from_stored_key() {
        let secrets = FakeSecretStore::with_api_key("rp_key_secret");
        let validator = FakeIdentityValidator::with_results([Ok(valid_identity())]);

        let setup = service(secrets.clone(), validator.clone())
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
        assert_eq!(validator.calls(), vec!["rp_key_secret".to_string()]);
    }

    #[tokio::test]
    async fn get_setup_reports_malformed_stored_key() {
        let secrets = FakeSecretStore::with_api_key(" \t");
        let validator = FakeIdentityValidator::with_results([]);

        let result = service(secrets, validator)
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
        let validator = FakeIdentityValidator::with_results([]);

        let result = service(secrets.clone(), validator.clone())
            .setup(DomainGpuCloudProviderId::Runpod, api_key("rp_new_secret"))
            .await;

        assert!(matches!(
            result,
            Err(ProviderSetupError::ProviderSetupAlreadyExists)
        ));
        assert!(validator.calls().is_empty());
        assert_eq!(secrets.stored_key(), Some("rp_existing_secret".to_string()));
        assert_eq!(secrets.calls(), vec![SecretStoreCall::ReadApiKey]);
    }

    #[tokio::test]
    async fn setup_rejects_unauthorized_submitted_key_without_mutating_keyring() {
        let secrets = FakeSecretStore::default();
        let validator = FakeIdentityValidator::with_results([Err(
            ProviderSetupError::ProviderApiKeyUnauthorized,
        )]);

        let result = service(secrets.clone(), validator.clone())
            .setup(DomainGpuCloudProviderId::Runpod, api_key("rp_bad_secret"))
            .await;

        assert!(matches!(
            result,
            Err(ProviderSetupError::ProviderApiKeyUnauthorized)
        ));
        assert_eq!(validator.calls(), vec!["rp_bad_secret".to_string()]);
        assert_eq!(secrets.stored_key(), None);
        assert_eq!(secrets.calls(), vec![SecretStoreCall::ReadApiKey]);
    }

    #[tokio::test]
    async fn setup_validates_writes_rereads_and_derives_setup_from_stored_key() {
        let secrets = FakeSecretStore::default();
        let validator = FakeIdentityValidator::with_results([
            Ok(ProviderIdentity {
                provider_user_email: "submitted@example.com".to_string(),
                provider_api_key_fingerprint: "submitted-key".to_string(),
            }),
            Ok(valid_identity()),
        ]);

        let setup = service(secrets.clone(), validator.clone())
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
            validator.calls(),
            vec!["rp_key_secret".to_string(), "rp_key_secret".to_string()]
        );
    }

    #[tokio::test]
    async fn setup_reports_keyring_write_failure_without_success() {
        let secrets = FakeSecretStore::default();
        secrets.set_replace_error(SecretStoreError::SecureKeyringUnavailable);
        let validator = FakeIdentityValidator::with_results([Ok(valid_identity())]);

        let result = service(secrets.clone(), validator)
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
        let validator = FakeIdentityValidator::with_results([Ok(valid_identity())]);

        let result = service(secrets.clone(), validator)
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
        let validator = FakeIdentityValidator::with_results([
            Ok(valid_identity()),
            Err(ProviderSetupError::ProviderApiUnavailable),
        ]);

        let result = service(secrets.clone(), validator)
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
        let validator = FakeIdentityValidator::with_results([
            Ok(valid_identity()),
            Err(ProviderSetupError::ProviderApiUnavailable),
        ]);

        let result = service(secrets.clone(), validator)
            .setup(DomainGpuCloudProviderId::Runpod, api_key("rp_key_secret"))
            .await;

        assert!(matches!(
            result,
            Err(ProviderSetupError::ProviderSetupRecoveryRequired)
        ));
        assert_eq!(secrets.stored_key(), Some("rp_key_secret".to_string()));
    }

    #[test]
    fn delete_setup_removes_existing_or_corrupt_local_entry_without_reading_provider_key() {
        let secrets = FakeSecretStore::with_api_key(" \t");
        let validator = FakeIdentityValidator::with_results([]);

        service(secrets.clone(), validator)
            .delete_setup(DomainGpuCloudProviderId::Runpod)
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

    #[test]
    fn delete_setup_reports_missing_setup() {
        let secrets = FakeSecretStore::default();
        let validator = FakeIdentityValidator::with_results([]);

        let result =
            service(secrets.clone(), validator).delete_setup(DomainGpuCloudProviderId::Runpod);

        assert_eq!(result, Err(ProviderSetupError::ProviderSetupNotFound));
        assert_eq!(secrets.calls(), vec![SecretStoreCall::HasApiKeyEntry]);
    }
}
