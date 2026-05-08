use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use crate::{
    domain::provider_setup::{GpuCloudProviderId, ProviderApiKey, ProviderIdentity},
    secrets::SecretStore,
};

use super::*;

#[derive(Debug, Clone)]
struct MemorySecretStore {
    key: Arc<Mutex<Option<String>>>,
    fail_read: bool,
    fail_replace: bool,
    fail_delete: bool,
}

impl MemorySecretStore {
    fn empty() -> Self {
        Self {
            key: Arc::new(Mutex::new(None)),
            fail_read: false,
            fail_replace: false,
            fail_delete: false,
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
    fn read_api_key(
        &self,
        _provider_id: &GpuCloudProviderId,
    ) -> Result<Option<ProviderApiKey>, ProviderSetupError> {
        if self.fail_read {
            return Err(ProviderSetupError::SecureKeyringUnavailable);
        }

        self.key
            .lock()
            .expect("memory store lock")
            .clone()
            .map(ProviderApiKey::new)
            .transpose()
            .map_err(|_| ProviderSetupError::InvalidProviderApiKey)
    }

    fn replace_api_key(
        &self,
        _provider_id: &GpuCloudProviderId,
        api_key: &ProviderApiKey,
    ) -> Result<(), ProviderSetupError> {
        if self.fail_replace {
            return Err(ProviderSetupError::SecureKeyringUnavailable);
        }

        *self.key.lock().expect("memory store lock") = Some(api_key.expose_secret().to_string());
        Ok(())
    }

    fn delete_api_key(&self, _provider_id: &GpuCloudProviderId) -> Result<(), ProviderSetupError> {
        if self.fail_delete {
            return Err(ProviderSetupError::SecureKeyringUnavailable);
        }

        *self.key.lock().expect("memory store lock") = None;
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct FakeProviderGateway {
    responses: HashMap<String, Result<ProviderIdentity, ProviderSetupError>>,
}

impl FakeProviderGateway {
    fn with_response(key: &str, response: Result<ProviderIdentity, ProviderSetupError>) -> Self {
        Self {
            responses: HashMap::from([(key.to_string(), response)]),
        }
    }
}

impl ProviderIdentityGateway for FakeProviderGateway {
    fn validate_identity<'a>(
        &'a self,
        _provider_id: &'a GpuCloudProviderId,
        api_key: &'a ProviderApiKey,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderIdentity, ProviderSetupError>> + Send + 'a>>
    {
        Box::pin(async move {
            self.responses
                .get(api_key.expose_secret())
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
