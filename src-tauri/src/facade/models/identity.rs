use serde::{Deserialize, Serialize};

use crate::application::secrets::Identity;

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct SetupApiKeyRequest {
    #[diagnostic(redact)]
    pub api_key: String,
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct IdentityDto {
    pub key_name: Option<String>,
    pub username: Option<String>,
    pub email: Option<String>,
}

impl From<Identity> for IdentityDto {
    fn from(value: Identity) -> Self {
        Self {
            key_name: value.key_name,
            username: value.username,
            email: value.email,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_key_request_redacts_the_raw_key_from_diagnostics() {
        let debug = format!(
            "{:?}",
            SetupApiKeyRequest {
                api_key: "raw-secret".into()
            }
        );

        assert!(!debug.contains("raw-secret"));
    }
}
