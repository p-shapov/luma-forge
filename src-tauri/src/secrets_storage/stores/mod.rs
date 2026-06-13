pub mod keyring;

use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

use crate::shared::AppFuture;

use super::errors::SecretsStorageError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SecretKey {
    #[serde(rename = "runpod")]
    RunpodApiKey,
    #[serde(rename = "hugging-face")]
    HuggingFaceApiKey,
}

impl SecretKey {
    pub(crate) fn storage_account_name(self) -> &'static str {
        match self {
            Self::RunpodApiKey => "runpod",
            Self::HuggingFaceApiKey => "hugging-face",
        }
    }
}

#[derive(Clone)]
pub struct ApiSecret(SecretString);

impl std::fmt::Debug for ApiSecret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let _secret = &self.0;

        formatter.write_str("ApiSecret([REDACTED])")
    }
}

impl ApiSecret {
    pub fn new(value: String) -> Result<Self, SecretsStorageError> {
        if value.trim().is_empty() {
            return Err(SecretsStorageError::SecretRequired);
        }

        Ok(Self(SecretString::from(value)))
    }

    pub(crate) fn expose_secret(&self) -> &str {
        self.0.expose_secret()
    }
}

pub trait SecretStore: Send + Sync {
    fn has<'a>(&'a self, key: SecretKey) -> AppFuture<'a, Result<bool, SecretsStorageError>>;

    fn write<'a>(
        &'a self,
        key: SecretKey,
        secret: ApiSecret,
    ) -> AppFuture<'a, Result<(), SecretsStorageError>>;

    fn delete<'a>(&'a self, key: SecretKey) -> AppFuture<'a, Result<(), SecretsStorageError>>;

    fn read<'a>(
        &'a self,
        key: SecretKey,
    ) -> AppFuture<'a, Result<Option<ApiSecret>, SecretsStorageError>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_secret_rejects_blank_values() {
        assert_eq!(
            ApiSecret::new(" \n\t".to_string()).map(|_| ()),
            Err(SecretsStorageError::SecretRequired)
        );
    }

    #[test]
    fn api_secret_debug_is_redacted() {
        let secret = ApiSecret::new("hf_secret_value".to_string()).expect("secret");
        let debug = format!("{secret:?}");

        assert_eq!(debug, "ApiSecret([REDACTED])");
        assert!(!debug.contains("hf_secret_value"));
        assert_eq!(secret.expose_secret(), "hf_secret_value");
    }

    #[test]
    fn secret_key_serializes_as_storage_account_identifier() {
        assert_eq!(SecretKey::RunpodApiKey.storage_account_name(), "runpod");
        assert_eq!(
            SecretKey::HuggingFaceApiKey.storage_account_name(),
            "hugging-face"
        );
        assert_eq!(
            serde_json::to_string(&SecretKey::RunpodApiKey).expect("secret key json"),
            "\"runpod\""
        );
        assert_eq!(
            serde_json::from_str::<SecretKey>("\"hugging-face\"").expect("secret key"),
            SecretKey::HuggingFaceApiKey
        );
    }
}
