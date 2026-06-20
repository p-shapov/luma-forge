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
