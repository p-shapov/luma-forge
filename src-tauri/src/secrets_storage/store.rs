use secrecy::{ExposeSecret, SecretString};

use crate::shared::AppFuture;

use super::errors::SecretsStorageError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretKey {
    RunpodApiKey,
    HuggingFaceApiKey,
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

    #[allow(dead_code)]
    pub(in crate::secrets_storage) fn expose_secret(&self) -> &str {
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
}
