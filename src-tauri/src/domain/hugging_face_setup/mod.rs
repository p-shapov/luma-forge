use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HuggingFaceApiKeySetup {
    pub api_key_fingerprint: String,
    pub user_name: String,
    pub user_email: Option<String>,
}

#[derive(Clone)]
pub struct HuggingFaceApiKey(SecretString);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HuggingFaceApiKeyError;

impl std::fmt::Debug for HuggingFaceApiKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("HuggingFaceApiKey([REDACTED])")
    }
}

impl HuggingFaceApiKey {
    pub fn new(value: String) -> Result<Self, HuggingFaceApiKeyError> {
        if value.trim().is_empty() {
            return Err(HuggingFaceApiKeyError);
        }

        Ok(Self(SecretString::from(value)))
    }

    pub fn expose_secret(&self) -> &str {
        self.0.expose_secret()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hugging_face_api_key_rejects_blank_values() {
        assert_eq!(
            HuggingFaceApiKey::new(" \n\t".to_string()).map(|_| ()),
            Err(HuggingFaceApiKeyError)
        );
    }

    #[test]
    fn hugging_face_api_key_exposes_secret_only_explicitly() {
        let key = HuggingFaceApiKey::new("hf_secret_value".to_string())
            .expect("hugging face key should be valid");

        assert_eq!(key.expose_secret(), "hf_secret_value");
        assert_eq!(format!("{key:?}"), "HuggingFaceApiKey([REDACTED])");
    }

    #[test]
    fn setup_identity_accepts_optional_email() {
        let setup = HuggingFaceApiKeySetup {
            api_key_fingerprint: "RUNPOD_READ".to_string(),
            user_name: "pavel".to_string(),
            user_email: None,
        };

        assert_eq!(setup.user_email, None);
    }
}
