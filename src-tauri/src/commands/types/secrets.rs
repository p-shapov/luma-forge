use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::secrets::ApiKeyIdentity;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SetupApiKeyRequest {
    pub api_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyIdentityResponse {
    pub email: Option<String>,
    pub username: Option<String>,
    pub key_display_name: Option<String>,
}

impl From<ApiKeyIdentity> for ApiKeyIdentityResponse {
    fn from(value: ApiKeyIdentity) -> Self {
        Self {
            email: value.email,
            username: value.username,
            key_display_name: value.key_display_name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_api_key_request_uses_camel_case() {
        let request = SetupApiKeyRequest {
            api_key: "secret-value".to_string(),
        };

        assert_eq!(
            serde_json::to_string(&request).expect("request json"),
            r#"{"apiKey":"secret-value"}"#
        );
    }

    #[test]
    fn identity_response_does_not_include_secret_value() {
        let response = ApiKeyIdentityResponse::from(ApiKeyIdentity {
            email: Some("user@example.test".to_string()),
            username: Some("user".to_string()),
            key_display_name: Some("display".to_string()),
        });

        let json = serde_json::to_string(&response).expect("identity json");

        assert!(json.contains("user@example.test"));
        assert!(!json.contains("secret"));
        assert!(!json.contains("apiKey"));
    }
}
