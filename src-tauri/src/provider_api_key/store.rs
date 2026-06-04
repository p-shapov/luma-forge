use std::fmt;

use crate::{domain::provider::GpuCloudProviderId, shared::AppFuture};

use super::error::ProviderApiKeyError;

#[derive(Clone, PartialEq, Eq)]
pub struct ProviderApiKey {
    raw: String,
}

impl ProviderApiKey {
    pub fn new(raw: impl Into<String>) -> Result<Self, ProviderApiKeyError> {
        let raw = raw.into();

        if raw.trim().is_empty() {
            return Err(ProviderApiKeyError::StoredProviderApiKeyInvalid);
        }

        Ok(Self { raw })
    }

    pub fn expose_secret(&self) -> &str {
        &self.raw
    }
}

impl fmt::Debug for ProviderApiKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderApiKey")
            .field("raw", &"<redacted>")
            .finish()
    }
}

pub trait ProviderApiKeyStore: Send + Sync {
    fn has_key<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
    ) -> AppFuture<'a, Result<bool, ProviderApiKeyError>>;

    fn read_key<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
    ) -> AppFuture<'a, Result<Option<ProviderApiKey>, ProviderApiKeyError>>;

    fn write_key<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        api_key: &'a ProviderApiKey,
    ) -> AppFuture<'a, Result<(), ProviderApiKeyError>>;

    fn remove_key<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
    ) -> AppFuture<'a, Result<(), ProviderApiKeyError>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_api_key_accepts_non_blank_values() {
        let api_key =
            ProviderApiKey::new("not-a-real-provider-key").expect("non-blank key should be valid");

        assert_eq!(api_key.expose_secret(), "not-a-real-provider-key");
    }

    #[test]
    fn provider_api_key_rejects_blank_values() {
        let error = ProviderApiKey::new("  \n\t").expect_err("blank key should be invalid");

        assert_eq!(error, ProviderApiKeyError::StoredProviderApiKeyInvalid);
    }

    #[test]
    fn provider_api_key_debug_output_is_redacted() {
        let api_key =
            ProviderApiKey::new("not-a-real-provider-key").expect("non-blank key should be valid");

        let debug = format!("{api_key:?}");

        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("not-a-real-provider-key"));
    }
}
