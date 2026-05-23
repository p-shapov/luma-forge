use std::fmt;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::hugging_face_setup as domain_hugging_face_setup;

#[allow(dead_code)]
mod remote_types {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, Type)]
    #[specta(remote = domain_hugging_face_setup::HuggingFaceApiKeySetup)]
    pub(super) struct HuggingFaceApiKeySetup {
        pub token_name: String,
        pub user_name: String,
        pub user_email: Option<String>,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetHuggingFaceApiKeySetupRequest;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetHuggingFaceApiKeySetupResponse {
    pub hugging_face_api_key_setup: Option<domain_hugging_face_setup::HuggingFaceApiKeySetup>,
}

#[derive(Clone, Serialize, Deserialize, Type)]
pub struct SetupHuggingFaceApiKeyRequest {
    pub hugging_face_api_key: String,
}

impl fmt::Debug for SetupHuggingFaceApiKeyRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SetupHuggingFaceApiKeyRequest")
            .field("hugging_face_api_key", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SetupHuggingFaceApiKeyResponse {
    pub hugging_face_api_key_setup: domain_hugging_face_setup::HuggingFaceApiKeySetup,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeleteHuggingFaceApiKeySetupRequest;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeleteHuggingFaceApiKeySetupResponse {
    pub hugging_face_api_key_setup: Option<domain_hugging_face_setup::HuggingFaceApiKeySetup>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_hugging_face_api_key_request_debug_redacts_api_key() {
        let request = SetupHuggingFaceApiKeyRequest {
            hugging_face_api_key: "hf_raw_secret_value".to_string(),
        };

        let debug = format!("{request:?}");

        assert!(debug.contains("hugging_face_api_key"));
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("hf_raw_secret_value"));
    }
}
