use std::{future::Future, pin::Pin};

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

use crate::{
    domain::provider_setup::{
        GpuCloudProviderId, GpuCloudProviderSetup, ProviderApiKey, ProviderIdentity,
    },
    secrets::SecretStore,
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetGpuCloudProviderSetupRequest {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetGpuCloudProviderSetupResponse {
    pub gpu_cloud_provider_setup: Option<GpuCloudProviderSetup>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SetupGpuCloudProviderRequest {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub provider_api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SetupGpuCloudProviderResponse {
    pub gpu_cloud_provider_setup: GpuCloudProviderSetup,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeleteGpuCloudProviderSetupRequest {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeleteGpuCloudProviderSetupResponse {
    pub gpu_cloud_provider_setup: Option<GpuCloudProviderSetup>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum NativeCommandErrorCode {
    UnsupportedProvider,
    ProviderSetupIncomplete,
    ProviderSetupAlreadyExists,
    InvalidProviderApiKey,
    ProviderApiUnavailable,
    ProviderIdentityUnavailable,
    SecureKeyringUnavailable,
    LocalStorageUnavailable,
    WorkflowCatalogUnavailable,
    WorkspaceCatalogUnavailable,
    InvalidPlacementPlan,
    WorkspaceAlreadyExists,
    InvalidRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NativeCommandError {
    pub code: NativeCommandErrorCode,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ProviderSetupError {
    #[error("provider setup is incomplete")]
    ProviderSetupIncomplete,
    #[error("provider setup already exists")]
    ProviderSetupAlreadyExists,
    #[error("invalid provider api key")]
    InvalidProviderApiKey,
    #[error("provider api unavailable")]
    ProviderApiUnavailable,
    #[error("provider identity unavailable")]
    ProviderIdentityUnavailable,
    #[error("secure keyring unavailable")]
    SecureKeyringUnavailable,
}

impl ProviderSetupError {
    pub fn code(&self) -> NativeCommandErrorCode {
        match self {
            Self::ProviderSetupIncomplete => NativeCommandErrorCode::ProviderSetupIncomplete,
            Self::ProviderSetupAlreadyExists => NativeCommandErrorCode::ProviderSetupAlreadyExists,
            Self::InvalidProviderApiKey => NativeCommandErrorCode::InvalidProviderApiKey,
            Self::ProviderApiUnavailable => NativeCommandErrorCode::ProviderApiUnavailable,
            Self::ProviderIdentityUnavailable => {
                NativeCommandErrorCode::ProviderIdentityUnavailable
            }
            Self::SecureKeyringUnavailable => NativeCommandErrorCode::SecureKeyringUnavailable,
        }
    }

    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Self::ProviderApiUnavailable | Self::SecureKeyringUnavailable
        )
    }

    pub fn ui_message(&self) -> &'static str {
        match self {
            Self::ProviderSetupIncomplete => "GPU cloud provider setup is incomplete.",
            Self::ProviderSetupAlreadyExists => "GPU cloud provider setup already exists.",
            Self::InvalidProviderApiKey => "Provider API key is invalid.",
            Self::ProviderApiUnavailable => "Provider API is unavailable.",
            Self::ProviderIdentityUnavailable => "Provider identity could not be verified.",
            Self::SecureKeyringUnavailable => "Secure keyring is unavailable.",
        }
    }
}

impl From<ProviderSetupError> for NativeCommandError {
    fn from(error: ProviderSetupError) -> Self {
        Self {
            code: error.code(),
            message: error.ui_message().to_string(),
            retryable: error.retryable(),
        }
    }
}

pub trait ProviderIdentityGateway: Send + Sync {
    fn validate_identity<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
        api_key: &'a ProviderApiKey,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderIdentity, ProviderSetupError>> + Send + 'a>>;
}

pub struct ProviderSetupService<S, P> {
    secrets: S,
    providers: P,
}

impl<S, P> ProviderSetupService<S, P> {
    pub fn new(secrets: S, providers: P) -> Self {
        Self { secrets, providers }
    }
}

impl<S, P> ProviderSetupService<S, P>
where
    S: SecretStore,
    P: ProviderIdentityGateway,
{
    pub async fn get_setup(
        &self,
        request: GetGpuCloudProviderSetupRequest,
    ) -> Result<GetGpuCloudProviderSetupResponse, ProviderSetupError> {
        let provider_id = request.gpu_cloud_provider_id;
        let Some(api_key) = self.secrets.read_api_key(&provider_id)? else {
            return Ok(GetGpuCloudProviderSetupResponse {
                gpu_cloud_provider_setup: None,
            });
        };

        let setup = self.setup_from_key(&provider_id, &api_key).await?;

        Ok(GetGpuCloudProviderSetupResponse {
            gpu_cloud_provider_setup: Some(setup),
        })
    }

    pub async fn setup(
        &self,
        request: SetupGpuCloudProviderRequest,
    ) -> Result<SetupGpuCloudProviderResponse, ProviderSetupError> {
        let provider_id = request.gpu_cloud_provider_id;
        if self.secrets.read_api_key(&provider_id)?.is_some() {
            return Err(ProviderSetupError::ProviderSetupAlreadyExists);
        }

        let api_key = ProviderApiKey::new(request.provider_api_key)?;

        self.setup_from_key(&provider_id, &api_key).await?;
        self.secrets.replace_api_key(&provider_id, &api_key)?;

        let stored_key = self
            .secrets
            .read_api_key(&provider_id)?
            .ok_or(ProviderSetupError::ProviderSetupIncomplete)?;
        let setup = self.setup_from_key(&provider_id, &stored_key).await?;

        Ok(SetupGpuCloudProviderResponse {
            gpu_cloud_provider_setup: setup,
        })
    }

    pub fn delete_setup(
        &self,
        request: DeleteGpuCloudProviderSetupRequest,
    ) -> Result<DeleteGpuCloudProviderSetupResponse, ProviderSetupError> {
        let provider_id = request.gpu_cloud_provider_id;
        if self.secrets.read_api_key(&provider_id)?.is_none() {
            return Err(ProviderSetupError::ProviderSetupIncomplete);
        }

        self.secrets.delete_api_key(&provider_id)?;

        Ok(DeleteGpuCloudProviderSetupResponse {
            gpu_cloud_provider_setup: None,
        })
    }

    async fn setup_from_key(
        &self,
        provider_id: &GpuCloudProviderId,
        api_key: &ProviderApiKey,
    ) -> Result<GpuCloudProviderSetup, ProviderSetupError> {
        let identity = self
            .providers
            .validate_identity(provider_id, api_key)
            .await?;

        Ok(GpuCloudProviderSetup {
            gpu_cloud_provider_id: *provider_id,
            provider_user_email: identity.provider_user_email,
            provider_api_key_fingerprint: identity.provider_api_key_fingerprint,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
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
        }

        fn replace_api_key(
            &self,
            _provider_id: &GpuCloudProviderId,
            api_key: &ProviderApiKey,
        ) -> Result<(), ProviderSetupError> {
            if self.fail_replace {
                return Err(ProviderSetupError::SecureKeyringUnavailable);
            }

            *self.key.lock().expect("memory store lock") =
                Some(api_key.expose_secret().to_string());
            Ok(())
        }

        fn delete_api_key(
            &self,
            _provider_id: &GpuCloudProviderId,
        ) -> Result<(), ProviderSetupError> {
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
        fn with_response(
            key: &str,
            response: Result<ProviderIdentity, ProviderSetupError>,
        ) -> Self {
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
}
