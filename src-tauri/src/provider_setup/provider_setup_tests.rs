use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{
    domain::provider_setup::{
        GpuCloudProviderId as DomainGpuCloudProviderId, ProviderApiKey, ProviderIdentity,
    },
    provider_setup::ProviderSetupCoordinator,
    secrets::{SecretStore, SecretStoreError},
    shared_contracts::provider_contracts::GpuCloudProviderId,
};

use super::*;

#[derive(Debug, Clone)]
struct MemorySecretStore {
    key: Arc<Mutex<Option<String>>>,
    fail_read: bool,
    fail_read_when_key_present: bool,
    fail_replace: bool,
    fail_delete: bool,
    replace_key_override: Option<String>,
}

impl MemorySecretStore {
    fn empty() -> Self {
        Self {
            key: Arc::new(Mutex::new(None)),
            fail_read: false,
            fail_read_when_key_present: false,
            fail_replace: false,
            fail_delete: false,
            replace_key_override: None,
        }
    }

    fn with_key(key: &str) -> Self {
        let store = Self::empty();
        *store.key.lock().expect("memory store lock") = Some(key.to_string());
        store
    }

    fn stored_key(&self) -> Option<String> {
        self.key.lock().expect("memory store lock").clone()
    }
}

impl SecretStore for MemorySecretStore {
    fn has_api_key_entry(
        &self,
        _provider_id: &DomainGpuCloudProviderId,
    ) -> Result<bool, SecretStoreError> {
        if self.fail_read {
            return Err(SecretStoreError::SecureKeyringUnavailable);
        }

        let key = self.key.lock().expect("memory store lock").clone();
        if self.fail_read_when_key_present && key.is_some() {
            return Err(SecretStoreError::SecureKeyringUnavailable);
        }

        Ok(key.is_some())
    }

    fn read_api_key(
        &self,
        _provider_id: &DomainGpuCloudProviderId,
    ) -> Result<Option<ProviderApiKey>, SecretStoreError> {
        if self.fail_read {
            return Err(SecretStoreError::SecureKeyringUnavailable);
        }

        let key = self.key.lock().expect("memory store lock").clone();
        if self.fail_read_when_key_present && key.is_some() {
            return Err(SecretStoreError::SecureKeyringUnavailable);
        }

        key.map(ProviderApiKey::new)
            .transpose()
            .map_err(|_| SecretStoreError::InvalidStoredProviderApiKey)
    }

    fn replace_api_key(
        &self,
        _provider_id: &DomainGpuCloudProviderId,
        api_key: &ProviderApiKey,
    ) -> Result<(), SecretStoreError> {
        if self.fail_replace {
            return Err(SecretStoreError::SecureKeyringUnavailable);
        }

        let stored_key = self
            .replace_key_override
            .clone()
            .unwrap_or_else(|| api_key.expose_secret().to_string());
        *self.key.lock().expect("memory store lock") = Some(stored_key);
        Ok(())
    }

    fn delete_api_key(
        &self,
        _provider_id: &DomainGpuCloudProviderId,
    ) -> Result<(), SecretStoreError> {
        if self.fail_delete {
            return Err(SecretStoreError::SecureKeyringUnavailable);
        }

        *self.key.lock().expect("memory store lock") = None;
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct FakeProviderGateway {
    responses: HashMap<String, Result<ProviderIdentity, ProviderSetupError>>,
    validation_count: Arc<Mutex<usize>>,
    validation_keys: Arc<Mutex<Vec<String>>>,
    delay: Option<Duration>,
}

impl FakeProviderGateway {
    fn with_response(key: &str, response: Result<ProviderIdentity, ProviderSetupError>) -> Self {
        Self::with_responses(HashMap::from([(key.to_string(), response)]))
    }

    fn with_responses(
        responses: HashMap<String, Result<ProviderIdentity, ProviderSetupError>>,
    ) -> Self {
        Self {
            responses,
            validation_count: Arc::new(Mutex::new(0)),
            validation_keys: Arc::new(Mutex::new(Vec::new())),
            delay: None,
        }
    }

    fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }

    fn validation_count(&self) -> usize {
        *self.validation_count.lock().expect("validation count lock")
    }

    fn validation_keys(&self) -> Vec<String> {
        self.validation_keys
            .lock()
            .expect("validation keys lock")
            .clone()
    }
}

impl ProviderIdentityGateway for FakeProviderGateway {
    fn validate_identity<'a>(
        &'a self,
        _provider_id: &'a DomainGpuCloudProviderId,
        api_key: &'a ProviderApiKey,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderIdentity, ProviderSetupError>> + Send + 'a>>
    {
        Box::pin(async move {
            let key = api_key.expose_secret().to_string();
            {
                *self.validation_count.lock().expect("validation count lock") += 1;
                self.validation_keys
                    .lock()
                    .expect("validation keys lock")
                    .push(key.clone());
            }
            if let Some(delay) = self.delay {
                tokio::time::sleep(delay).await;
            }

            self.responses
                .get(&key)
                .cloned()
                .unwrap_or(Err(ProviderSetupError::InvalidProviderApiKey))
        })
    }
}

fn identity(fingerprint: &str) -> ProviderIdentity {
    ProviderIdentity {
        provider_user_email: "user@example.com".to_string(),
        provider_api_key_fingerprint: fingerprint.to_string(),
    }
}

fn setup_request(provider_api_key: &str) -> SetupGpuCloudProviderRequest {
    SetupGpuCloudProviderRequest {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        provider_api_key: provider_api_key.to_string(),
    }
}

async fn setup_with_coordinator(
    coordinator: Arc<ProviderSetupCoordinator>,
    store: MemorySecretStore,
    providers: FakeProviderGateway,
    provider_api_key: &str,
) -> Result<SetupGpuCloudProviderResponse, ProviderSetupError> {
    let provider_id = DomainGpuCloudProviderId::Runpod;
    let _guard = coordinator.lock(&provider_id).await;
    ProviderSetupService::new(store, providers)
        .setup(setup_request(provider_api_key))
        .await
}

async fn delete_with_coordinator(
    coordinator: Arc<ProviderSetupCoordinator>,
    store: MemorySecretStore,
    providers: FakeProviderGateway,
) -> Result<DeleteGpuCloudProviderSetupResponse, ProviderSetupError> {
    let provider_id = DomainGpuCloudProviderId::Runpod;
    let _guard = coordinator.lock(&provider_id).await;
    ProviderSetupService::new(store, providers).delete_setup(DeleteGpuCloudProviderSetupRequest {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
    })
}

async fn wait_for_validation_count(providers: &FakeProviderGateway, expected: usize) {
    for _ in 0..50 {
        if providers.validation_count() >= expected {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    panic!(
        "expected at least {expected} validations, got {}",
        providers.validation_count()
    );
}

#[tokio::test]
async fn get_setup_returns_null_when_key_is_missing() {
    let service = ProviderSetupService::new(
        MemorySecretStore::empty(),
        FakeProviderGateway::with_response("unused", Ok(identity("rp_123"))),
    );

    let response = service
        .get_setup(GetGpuCloudProviderSetupRequest {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        })
        .await
        .expect("get setup should succeed");

    assert!(response.gpu_cloud_provider_setup.is_none());
}

#[tokio::test]
async fn get_setup_returns_live_status_for_valid_stored_key() {
    let service = ProviderSetupService::new(
        MemorySecretStore::with_key("rp_123_secret"),
        FakeProviderGateway::with_response("rp_123_secret", Ok(identity("rp_123"))),
    );

    let response = service
        .get_setup(GetGpuCloudProviderSetupRequest {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        })
        .await
        .expect("get setup should succeed");

    assert_eq!(
        response
            .gpu_cloud_provider_setup
            .expect("setup should exist")
            .provider_api_key_fingerprint,
        "rp_123"
    );
}

#[tokio::test]
async fn get_setup_maps_invalid_stored_key() {
    let service = ProviderSetupService::new(
        MemorySecretStore::with_key("bad-key"),
        FakeProviderGateway::with_response(
            "bad-key",
            Err(ProviderSetupError::InvalidProviderApiKey),
        ),
    );

    let error = service
        .get_setup(GetGpuCloudProviderSetupRequest {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        })
        .await
        .expect_err("invalid key should fail");

    assert_eq!(error, ProviderSetupError::InvalidProviderApiKey);
}

#[tokio::test]
async fn get_setup_maps_provider_api_unavailable() {
    let service = ProviderSetupService::new(
        MemorySecretStore::with_key("stored-key"),
        FakeProviderGateway::with_response(
            "stored-key",
            Err(ProviderSetupError::ProviderApiUnavailable),
        ),
    );

    let error = service
        .get_setup(GetGpuCloudProviderSetupRequest {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        })
        .await
        .expect_err("provider outage should fail");

    assert_eq!(error, ProviderSetupError::ProviderApiUnavailable);
}

#[tokio::test]
async fn setup_stores_new_key_after_validation() {
    let store = MemorySecretStore::empty();
    let service = ProviderSetupService::new(
        store.clone(),
        FakeProviderGateway::with_response("new-key", Ok(identity("new"))),
    );

    let response = service
        .setup(SetupGpuCloudProviderRequest {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            provider_api_key: "new-key".to_string(),
        })
        .await
        .expect("valid setup should succeed");

    assert_eq!(
        response
            .gpu_cloud_provider_setup
            .provider_api_key_fingerprint,
        "new"
    );
    assert_eq!(store.stored_key(), Some("new-key".to_string()));
}

#[tokio::test]
async fn setup_revalidates_stored_key_after_writing() {
    let store = MemorySecretStore::empty();
    let providers = FakeProviderGateway::with_response("new-key", Ok(identity("new")));
    let service = ProviderSetupService::new(store.clone(), providers.clone());

    let response = service
        .setup(SetupGpuCloudProviderRequest {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            provider_api_key: "new-key".to_string(),
        })
        .await
        .expect("valid setup should succeed");

    assert_eq!(
        response
            .gpu_cloud_provider_setup
            .provider_api_key_fingerprint,
        "new"
    );
    assert_eq!(store.stored_key(), Some("new-key".to_string()));
    assert_eq!(providers.validation_count(), 2);
}

#[tokio::test]
async fn setup_returns_status_from_re_read_stored_key() {
    let mut store = MemorySecretStore::empty();
    store.replace_key_override = Some("stored-key".to_string());
    let providers = FakeProviderGateway::with_responses(HashMap::from([
        ("new-key".to_string(), Ok(identity("submitted"))),
        ("stored-key".to_string(), Ok(identity("stored"))),
    ]));
    let service = ProviderSetupService::new(store.clone(), providers.clone());

    let response = service
        .setup(setup_request("new-key"))
        .await
        .expect("valid setup should succeed");

    assert_eq!(
        response
            .gpu_cloud_provider_setup
            .provider_api_key_fingerprint,
        "stored"
    );
    assert_eq!(store.stored_key(), Some("stored-key".to_string()));
    assert_eq!(
        providers.validation_keys(),
        vec!["new-key".to_string(), "stored-key".to_string()]
    );
}

#[tokio::test]
async fn setup_maps_stored_key_re_read_failure() {
    let mut store = MemorySecretStore::empty();
    store.fail_read_when_key_present = true;
    let service = ProviderSetupService::new(
        store.clone(),
        FakeProviderGateway::with_response("new-key", Ok(identity("new"))),
    );

    let error = service
        .setup(setup_request("new-key"))
        .await
        .expect_err("re-read failure should fail setup");

    assert_eq!(error, ProviderSetupError::SecureKeyringUnavailable);
    assert_eq!(store.stored_key(), Some("new-key".to_string()));
}

#[tokio::test]
async fn setup_rejects_existing_setup_before_validating_submitted_key() {
    let store = MemorySecretStore::with_key("old-key");
    let service = ProviderSetupService::new(
        store.clone(),
        FakeProviderGateway::with_response("old-key", Ok(identity("old"))),
    );

    let error = service
        .setup(SetupGpuCloudProviderRequest {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            provider_api_key: " ".to_string(),
        })
        .await
        .expect_err("repeated setup should fail");

    assert_eq!(error, ProviderSetupError::ProviderSetupAlreadyExists);
    assert_eq!(store.stored_key(), Some("old-key".to_string()));
}

#[tokio::test]
async fn concurrent_setup_requests_create_at_most_one_setup() {
    let store = MemorySecretStore::empty();
    let providers = FakeProviderGateway::with_responses(HashMap::from([
        ("first-key".to_string(), Ok(identity("first"))),
        ("second-key".to_string(), Ok(identity("second"))),
    ]))
    .with_delay(Duration::from_millis(50));
    let coordinator = Arc::new(ProviderSetupCoordinator::default());

    let first = tokio::spawn(setup_with_coordinator(
        coordinator.clone(),
        store.clone(),
        providers.clone(),
        "first-key",
    ));
    wait_for_validation_count(&providers, 1).await;
    let second = tokio::spawn(setup_with_coordinator(
        coordinator,
        store.clone(),
        providers.clone(),
        "second-key",
    ));

    let first_result = first.await.expect("first task should join");
    let second_result = second.await.expect("second task should join");

    assert!(first_result.is_ok());
    assert_eq!(
        second_result.expect_err("second setup should fail"),
        ProviderSetupError::ProviderSetupAlreadyExists
    );
    assert_eq!(store.stored_key(), Some("first-key".to_string()));
}

#[tokio::test]
async fn later_concurrent_setup_does_not_validate_submitted_key() {
    let store = MemorySecretStore::empty();
    let providers = FakeProviderGateway::with_responses(HashMap::from([
        ("first-key".to_string(), Ok(identity("first"))),
        ("second-key".to_string(), Ok(identity("second"))),
    ]))
    .with_delay(Duration::from_millis(50));
    let coordinator = Arc::new(ProviderSetupCoordinator::default());

    let first = tokio::spawn(setup_with_coordinator(
        coordinator.clone(),
        store.clone(),
        providers.clone(),
        "first-key",
    ));
    wait_for_validation_count(&providers, 1).await;
    let second = tokio::spawn(setup_with_coordinator(
        coordinator,
        store,
        providers.clone(),
        "second-key",
    ));

    first
        .await
        .expect("first task should join")
        .expect("first setup");
    let error = second
        .await
        .expect("second task should join")
        .expect_err("second setup should fail");

    assert_eq!(error, ProviderSetupError::ProviderSetupAlreadyExists);
    assert_eq!(
        providers.validation_keys(),
        vec!["first-key".to_string(), "first-key".to_string()]
    );
}

#[tokio::test]
async fn setup_does_not_mutate_keyring_when_submitted_key_is_invalid() {
    let store = MemorySecretStore::empty();
    let service = ProviderSetupService::new(
        store.clone(),
        FakeProviderGateway::with_response(
            "new-key",
            Err(ProviderSetupError::InvalidProviderApiKey),
        ),
    );

    let error = service
        .setup(SetupGpuCloudProviderRequest {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            provider_api_key: "new-key".to_string(),
        })
        .await
        .expect_err("invalid setup should fail");

    assert_eq!(error, ProviderSetupError::InvalidProviderApiKey);
    assert_eq!(store.stored_key(), None);
}

#[tokio::test]
async fn setup_maps_keyring_write_failure() {
    let mut store = MemorySecretStore::empty();
    store.fail_replace = true;
    let service = ProviderSetupService::new(
        store.clone(),
        FakeProviderGateway::with_response("new-key", Ok(identity("new"))),
    );

    let error = service
        .setup(SetupGpuCloudProviderRequest {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            provider_api_key: "new-key".to_string(),
        })
        .await
        .expect_err("write failure should fail");

    assert_eq!(error, ProviderSetupError::SecureKeyringUnavailable);
    assert_eq!(store.stored_key(), None);
}

#[tokio::test]
async fn setup_rejects_empty_key_without_mutation() {
    let store = MemorySecretStore::empty();
    let service = ProviderSetupService::new(
        store.clone(),
        FakeProviderGateway::with_response("unused", Ok(identity("unused"))),
    );

    let error = service
        .setup(SetupGpuCloudProviderRequest {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            provider_api_key: " ".to_string(),
        })
        .await
        .expect_err("empty key should fail");

    assert_eq!(error, ProviderSetupError::InvalidProviderApiKey);
    assert_eq!(store.stored_key(), None);
}

#[tokio::test]
async fn setup_maps_provider_identity_unavailable() {
    let store = MemorySecretStore::empty();
    let service = ProviderSetupService::new(
        store.clone(),
        FakeProviderGateway::with_response(
            "new-key",
            Err(ProviderSetupError::ProviderIdentityUnavailable),
        ),
    );

    let error = service
        .setup(SetupGpuCloudProviderRequest {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            provider_api_key: "new-key".to_string(),
        })
        .await
        .expect_err("identity mismatch should fail");

    assert_eq!(error, ProviderSetupError::ProviderIdentityUnavailable);
    assert_eq!(store.stored_key(), None);
}

#[test]
fn delete_setup_removes_existing_key() {
    let store = MemorySecretStore::with_key("stored-key");
    let service = ProviderSetupService::new(
        store.clone(),
        FakeProviderGateway::with_response("stored-key", Ok(identity("stored"))),
    );

    let response = service
        .delete_setup(DeleteGpuCloudProviderSetupRequest {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        })
        .expect("delete should succeed");

    assert!(response.gpu_cloud_provider_setup.is_none());
    assert_eq!(store.stored_key(), None);
}

#[test]
fn delete_setup_removes_corrupt_stored_key() {
    let store = MemorySecretStore::with_key("");
    let service = ProviderSetupService::new(
        store.clone(),
        FakeProviderGateway::with_response("unused", Ok(identity("unused"))),
    );

    let response = service
        .delete_setup(DeleteGpuCloudProviderSetupRequest {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        })
        .expect("delete should recover corrupt local setup");

    assert!(response.gpu_cloud_provider_setup.is_none());
    assert_eq!(store.stored_key(), None);
}

#[test]
fn delete_setup_errors_when_key_is_missing() {
    let service = ProviderSetupService::new(
        MemorySecretStore::empty(),
        FakeProviderGateway::with_response("unused", Ok(identity("unused"))),
    );

    let error = service
        .delete_setup(DeleteGpuCloudProviderSetupRequest {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        })
        .expect_err("missing setup should fail delete");

    assert_eq!(error, ProviderSetupError::ProviderSetupIncomplete);
}

#[test]
fn delete_setup_maps_keyring_entry_lookup_failure() {
    let mut store = MemorySecretStore::with_key("stored-key");
    store.fail_read = true;
    let service = ProviderSetupService::new(
        store,
        FakeProviderGateway::with_response("stored-key", Ok(identity("stored"))),
    );

    let error = service
        .delete_setup(DeleteGpuCloudProviderSetupRequest {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        })
        .expect_err("entry lookup failure should fail delete");

    assert_eq!(error, ProviderSetupError::SecureKeyringUnavailable);
}

#[test]
fn delete_setup_maps_keyring_failure() {
    let mut store = MemorySecretStore::with_key("stored-key");
    store.fail_delete = true;
    let service = ProviderSetupService::new(
        store,
        FakeProviderGateway::with_response("stored-key", Ok(identity("stored"))),
    );

    let error = service
        .delete_setup(DeleteGpuCloudProviderSetupRequest {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        })
        .expect_err("delete failure should fail");

    assert_eq!(error, ProviderSetupError::SecureKeyringUnavailable);
}

#[tokio::test]
async fn delete_waits_for_concurrent_setup_to_finish() {
    let store = MemorySecretStore::empty();
    let providers = FakeProviderGateway::with_response("new-key", Ok(identity("new")))
        .with_delay(Duration::from_millis(50));
    let coordinator = Arc::new(ProviderSetupCoordinator::default());

    let setup = tokio::spawn(setup_with_coordinator(
        coordinator.clone(),
        store.clone(),
        providers.clone(),
        "new-key",
    ));
    wait_for_validation_count(&providers, 1).await;
    let delete = tokio::spawn(delete_with_coordinator(
        coordinator,
        store.clone(),
        providers.clone(),
    ));

    setup
        .await
        .expect("setup task should join")
        .expect("setup should succeed");
    delete
        .await
        .expect("delete task should join")
        .expect("delete should succeed after setup");

    assert_eq!(store.stored_key(), None);
}
