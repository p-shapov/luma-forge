use keyring::{Entry, Error as KeyringError};
use secrecy::{ExposeSecret, SecretString};

use crate::domain::provider_setup::{GpuCloudProviderId, ProviderApiKey};

use super::SecretStoreError;

const GPU_CLOUD_PROVIDER_KEYRING_SCOPE: &str = "gpu-cloud-provider";
const PROVISIONER_WORKER_KEYRING_SCOPE: &str = "provisioner-worker";

#[derive(Clone)]
pub struct ProvisionerWorkerBearerToken(SecretString);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionerWorkerBearerTokenError;

impl std::fmt::Debug for ProvisionerWorkerBearerToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ProvisionerWorkerBearerToken([REDACTED])")
    }
}

impl ProvisionerWorkerBearerToken {
    pub fn new(value: String) -> Result<Self, ProvisionerWorkerBearerTokenError> {
        if value.trim().is_empty() {
            return Err(ProvisionerWorkerBearerTokenError);
        }

        Ok(Self(SecretString::from(value)))
    }

    pub fn expose_secret(&self) -> &str {
        self.0.expose_secret()
    }
}

pub trait SecretStore: Send + Sync {
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

    fn write_provisioner_worker_token(
        &self,
        workspace_id: &str,
        token: &ProvisionerWorkerBearerToken,
    ) -> Result<(), SecretStoreError>;

    fn read_provisioner_worker_token(
        &self,
        workspace_id: &str,
    ) -> Result<Option<ProvisionerWorkerBearerToken>, SecretStoreError>;

    fn delete_provisioner_worker_token(&self, workspace_id: &str) -> Result<(), SecretStoreError>;
}

#[derive(Debug, Clone)]
pub struct KeyringSecretStore {
    provider_service_name: String,
    provisioner_worker_service_name: String,
}

impl KeyringSecretStore {
    pub fn new(app_identifier: impl AsRef<str>) -> Self {
        Self {
            provider_service_name: format!(
                "{}.{GPU_CLOUD_PROVIDER_KEYRING_SCOPE}",
                app_identifier.as_ref()
            ),
            provisioner_worker_service_name: format!(
                "{}.{PROVISIONER_WORKER_KEYRING_SCOPE}",
                app_identifier.as_ref()
            ),
        }
    }

    fn provider_api_key_entry(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> Result<Entry, SecretStoreError> {
        Entry::new(&self.provider_service_name, keyring_account(provider_id))
            .map_err(|_| SecretStoreError::SecureKeyringUnavailable)
    }

    fn provisioner_worker_entry(&self, workspace_id: &str) -> Result<Entry, SecretStoreError> {
        Entry::new(
            &self.provisioner_worker_service_name,
            &provisioner_worker_account(workspace_id),
        )
        .map_err(|_| SecretStoreError::SecureKeyringUnavailable)
    }
}

fn keyring_account(provider_id: &GpuCloudProviderId) -> &'static str {
    match provider_id {
        GpuCloudProviderId::Runpod => "runpod",
    }
}

fn provisioner_worker_account(workspace_id: &str) -> String {
    format!("workspace:{workspace_id}")
}

impl SecretStore for KeyringSecretStore {
    fn has_api_key_entry(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> Result<bool, SecretStoreError> {
        match self.provider_api_key_entry(provider_id)?.get_password() {
            Ok(_) => Ok(true),
            Err(KeyringError::NoEntry) => Ok(false),
            Err(_) => Err(SecretStoreError::SecureKeyringUnavailable),
        }
    }

    fn read_api_key(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> Result<Option<ProviderApiKey>, SecretStoreError> {
        match self.provider_api_key_entry(provider_id)?.get_password() {
            Ok(api_key) => ProviderApiKey::new(api_key)
                .map(Some)
                .map_err(|_| SecretStoreError::InvalidStoredProviderApiKey),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(_) => Err(SecretStoreError::SecureKeyringUnavailable),
        }
    }

    fn replace_api_key(
        &self,
        provider_id: &GpuCloudProviderId,
        api_key: &ProviderApiKey,
    ) -> Result<(), SecretStoreError> {
        self.provider_api_key_entry(provider_id)?
            .set_password(api_key.expose_secret())
            .map_err(|_| SecretStoreError::SecureKeyringUnavailable)
    }

    fn delete_api_key(&self, provider_id: &GpuCloudProviderId) -> Result<(), SecretStoreError> {
        self.provider_api_key_entry(provider_id)?
            .delete_credential()
            .map_err(|_| SecretStoreError::SecureKeyringUnavailable)
    }

    fn write_provisioner_worker_token(
        &self,
        workspace_id: &str,
        token: &ProvisionerWorkerBearerToken,
    ) -> Result<(), SecretStoreError> {
        self.provisioner_worker_entry(workspace_id)?
            .set_password(token.expose_secret())
            .map_err(|_| SecretStoreError::SecureKeyringUnavailable)
    }

    fn read_provisioner_worker_token(
        &self,
        workspace_id: &str,
    ) -> Result<Option<ProvisionerWorkerBearerToken>, SecretStoreError> {
        match self.provisioner_worker_entry(workspace_id)?.get_password() {
            Ok(token) => ProvisionerWorkerBearerToken::new(token)
                .map(Some)
                .map_err(|_| SecretStoreError::InvalidStoredProvisionerWorkerToken),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(_) => Err(SecretStoreError::SecureKeyringUnavailable),
        }
    }

    fn delete_provisioner_worker_token(&self, workspace_id: &str) -> Result<(), SecretStoreError> {
        match self
            .provisioner_worker_entry(workspace_id)?
            .delete_credential()
        {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(_) => Err(SecretStoreError::SecureKeyringUnavailable),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keyring::{
        credential::{
            Credential, CredentialApi, CredentialBuilder, CredentialBuilderApi,
            CredentialPersistence,
        },
        set_default_credential_builder, Error as KeyringError,
    };
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex, OnceLock},
    };

    #[derive(Debug, Default, Clone)]
    struct SharedCredentialBuilder {
        state: Arc<Mutex<SharedCredentialState>>,
    }

    #[derive(Debug, Default)]
    struct SharedCredentialState {
        secrets: HashMap<(String, String), Vec<u8>>,
        next_errors: HashMap<(String, String), KeyringError>,
    }

    impl SharedCredentialBuilder {
        fn set_next_error(&self, service: &str, user: &str, error: KeyringError) {
            self.state
                .lock()
                .expect("shared credential state")
                .next_errors
                .insert((service.to_string(), user.to_string()), error);
        }
    }

    impl CredentialBuilderApi for SharedCredentialBuilder {
        fn build(
            &self,
            _target: Option<&str>,
            service: &str,
            user: &str,
        ) -> keyring::Result<Box<Credential>> {
            Ok(Box::new(SharedCredential {
                service: service.to_string(),
                user: user.to_string(),
                state: Arc::clone(&self.state),
            }))
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn persistence(&self) -> CredentialPersistence {
            CredentialPersistence::ProcessOnly
        }
    }

    #[derive(Debug)]
    struct SharedCredential {
        service: String,
        user: String,
        state: Arc<Mutex<SharedCredentialState>>,
    }

    impl SharedCredential {
        fn key(&self) -> (String, String) {
            (self.service.clone(), self.user.clone())
        }

        fn take_next_error(
            state: &mut SharedCredentialState,
            key: &(String, String),
        ) -> Option<KeyringError> {
            state.next_errors.remove(key)
        }
    }

    impl CredentialApi for SharedCredential {
        fn set_secret(&self, secret: &[u8]) -> keyring::Result<()> {
            let key = self.key();
            let mut state = self.state.lock().expect("shared credential state");
            if let Some(error) = Self::take_next_error(&mut state, &key) {
                return Err(error);
            }

            state.secrets.insert(key, secret.to_vec());
            Ok(())
        }

        fn get_secret(&self) -> keyring::Result<Vec<u8>> {
            let key = self.key();
            let mut state = self.state.lock().expect("shared credential state");
            if let Some(error) = Self::take_next_error(&mut state, &key) {
                return Err(error);
            }

            state
                .secrets
                .get(&key)
                .cloned()
                .ok_or(KeyringError::NoEntry)
        }

        fn delete_credential(&self) -> keyring::Result<()> {
            let key = self.key();
            let mut state = self.state.lock().expect("shared credential state");
            if let Some(error) = Self::take_next_error(&mut state, &key) {
                return Err(error);
            }

            state
                .secrets
                .remove(&key)
                .map(|_| ())
                .ok_or(KeyringError::NoEntry)
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    fn keyring_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn with_keyring_store(test: impl FnOnce(KeyringSecretStore, SharedCredentialBuilder)) {
        let _guard = keyring_test_lock().lock().expect("keyring test lock");
        let builder = SharedCredentialBuilder::default();
        set_default_credential_builder(Box::new(builder.clone()) as Box<CredentialBuilder>);
        test(KeyringSecretStore::new("com.luma-forge.test"), builder);
    }

    fn provider_key(value: &str) -> ProviderApiKey {
        ProviderApiKey::new(value.to_string()).expect("provider key should be valid")
    }

    fn worker_token(value: &str) -> ProvisionerWorkerBearerToken {
        ProvisionerWorkerBearerToken::new(value.to_string()).expect("worker token should be valid")
    }

    #[test]
    fn provisioner_worker_bearer_token_rejects_blank_values() {
        assert_eq!(
            ProvisionerWorkerBearerToken::new(" \n\t".to_string()).map(|_| ()),
            Err(ProvisionerWorkerBearerTokenError)
        );
    }

    #[test]
    fn provisioner_worker_bearer_token_exposes_secret_only_explicitly() {
        let token = worker_token("worker-secret");

        assert_eq!(token.expose_secret(), "worker-secret");
        assert_eq!(
            format!("{token:?}"),
            "ProvisionerWorkerBearerToken([REDACTED])"
        );
    }

    #[test]
    fn keyring_secret_store_reads_none_when_provider_key_is_missing() {
        with_keyring_store(|store, _builder| {
            assert!(store
                .read_api_key(&GpuCloudProviderId::Runpod)
                .expect("read provider key should succeed")
                .is_none());
            assert_eq!(
                store.has_api_key_entry(&GpuCloudProviderId::Runpod),
                Ok(false)
            );
        });
    }

    #[test]
    fn keyring_secret_store_replaces_reads_and_deletes_provider_key() {
        with_keyring_store(|store, _builder| {
            store
                .replace_api_key(&GpuCloudProviderId::Runpod, &provider_key("rp_secret"))
                .expect("replace provider key should succeed");

            assert_eq!(
                store.has_api_key_entry(&GpuCloudProviderId::Runpod),
                Ok(true)
            );
            assert_eq!(
                store
                    .read_api_key(&GpuCloudProviderId::Runpod)
                    .expect("read provider key should succeed")
                    .expect("provider key should exist")
                    .expose_secret(),
                "rp_secret"
            );

            store
                .delete_api_key(&GpuCloudProviderId::Runpod)
                .expect("delete provider key should succeed");
            assert!(store
                .read_api_key(&GpuCloudProviderId::Runpod)
                .expect("read provider key should succeed")
                .is_none());
        });
    }

    #[test]
    fn keyring_secret_store_reports_invalid_stored_provider_key() {
        with_keyring_store(|store, _builder| {
            store
                .provider_api_key_entry(&GpuCloudProviderId::Runpod)
                .expect("provider key entry should be created")
                .set_password(" \t")
                .expect("test fixture should seed malformed provider key");

            assert!(matches!(
                store.read_api_key(&GpuCloudProviderId::Runpod),
                Err(SecretStoreError::InvalidStoredProviderApiKey)
            ));
        });
    }

    #[test]
    fn keyring_secret_store_writes_reads_and_deletes_worker_token() {
        with_keyring_store(|store, _builder| {
            store
                .write_provisioner_worker_token("workspace-1", &worker_token("worker-secret"))
                .expect("write worker token should succeed");

            assert_eq!(
                store
                    .read_provisioner_worker_token("workspace-1")
                    .expect("read worker token should succeed")
                    .expect("worker token should exist")
                    .expose_secret(),
                "worker-secret"
            );

            store
                .delete_provisioner_worker_token("workspace-1")
                .expect("delete worker token should succeed");
            assert!(store
                .read_provisioner_worker_token("workspace-1")
                .expect("read worker token should succeed")
                .is_none());
        });
    }

    #[test]
    fn keyring_secret_store_reports_invalid_stored_worker_token() {
        with_keyring_store(|store, _builder| {
            store
                .provisioner_worker_entry("workspace-1")
                .expect("worker token entry should be created")
                .set_password(" \t")
                .expect("test fixture should seed malformed worker token");

            assert!(matches!(
                store.read_provisioner_worker_token("workspace-1"),
                Err(SecretStoreError::InvalidStoredProvisionerWorkerToken)
            ));
        });
    }

    #[test]
    fn keyring_secret_store_deletes_missing_worker_token_idempotently() {
        with_keyring_store(|store, _builder| {
            assert_eq!(store.delete_provisioner_worker_token("workspace-1"), Ok(()));
        });
    }

    #[test]
    fn keyring_secret_store_maps_keyring_errors_to_unavailable() {
        with_keyring_store(|store, builder| {
            builder.set_next_error(
                &store.provider_service_name,
                keyring_account(&GpuCloudProviderId::Runpod),
                KeyringError::Invalid("mock".to_string(), "failure".to_string()),
            );

            assert!(matches!(
                store.read_api_key(&GpuCloudProviderId::Runpod),
                Err(SecretStoreError::SecureKeyringUnavailable)
            ));
        });
    }
}
