use secrecy::SecretString;

use crate::{
    application::secrets::{SecretKind, SecretStore, SecretStoreError},
    infra::keyring::{KeyringStorage, KeyringStorageError},
};

const RUNPOD_ACCOUNT: &str = "runpod-api-key";
const HUGGING_FACE_ACCOUNT: &str = "hugging-face-api-key";

pub struct KeyringSecretStore {
    storage: KeyringStorage,
}

impl KeyringSecretStore {
    pub fn new(storage: KeyringStorage) -> Self {
        Self { storage }
    }
}

#[async_trait::async_trait]
impl SecretStore for KeyringSecretStore {
    async fn exists(&self, kind: SecretKind) -> Result<bool, SecretStoreError> {
        self.storage
            .exists(account(kind))
            .await
            .map_err(map_storage_error)
    }

    async fn get(&self, kind: SecretKind) -> Result<Option<SecretString>, SecretStoreError> {
        self.storage
            .get(account(kind))
            .await
            .map_err(map_storage_error)
    }

    async fn insert(&self, kind: SecretKind, secret: SecretString) -> Result<(), SecretStoreError> {
        if self.exists(kind).await? {
            return Err(SecretStoreError::AlreadyExists);
        }
        self.storage
            .set(account(kind), secret)
            .await
            .map_err(map_storage_error)
    }

    async fn delete(&self, kind: SecretKind) -> Result<(), SecretStoreError> {
        if !self.exists(kind).await? {
            return Err(SecretStoreError::NotFound);
        }
        self.storage
            .delete(account(kind))
            .await
            .map_err(map_storage_error)
    }
}

fn account(kind: SecretKind) -> &'static str {
    match kind {
        SecretKind::RunpodApiKey => RUNPOD_ACCOUNT,
        SecretKind::HuggingFaceApiKey => HUGGING_FACE_ACCOUNT,
    }
}

fn map_storage_error(_: KeyringStorageError) -> SecretStoreError {
    SecretStoreError::Unavailable
}
