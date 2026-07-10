use std::time::Duration;

use reqwest::StatusCode;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

use super::HuggingFaceError;

const WHOAMI_URL: &str = "https://huggingface.co/api/whoami-v2";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HuggingFaceIdentity {
    pub username: String,
    pub email: Option<String>,
    pub token_display_name: Option<String>,
}

#[derive(Clone)]
pub struct HuggingFaceClient {
    http: reqwest::Client,
}

impl HuggingFaceClient {
    pub fn new() -> Result<Self, HuggingFaceError> {
        let http = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|_| HuggingFaceError::RequestFailed)?;

        Ok(Self { http })
    }

    pub async fn identity(
        &self,
        token: &SecretString,
    ) -> Result<HuggingFaceIdentity, HuggingFaceError> {
        let response = self
            .http
            .get(WHOAMI_URL)
            .bearer_auth(token.expose_secret())
            .send()
            .await
            .map_err(transport_error)?;

        if let Some(error) = status_error(response.status()) {
            return Err(error);
        }

        let response = response
            .json::<WhoamiResponse>()
            .await
            .map_err(|_| HuggingFaceError::InvalidResponse)?;

        map_identity(response)
    }
}

#[derive(Deserialize)]
struct WhoamiResponse {
    name: String,
    email: Option<String>,
    auth: Option<WhoamiAuth>,
}

#[derive(Deserialize)]
struct WhoamiAuth {
    #[serde(rename = "accessToken")]
    access_token: Option<WhoamiAccessToken>,
}

#[derive(Deserialize)]
struct WhoamiAccessToken {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

fn map_identity(response: WhoamiResponse) -> Result<HuggingFaceIdentity, HuggingFaceError> {
    let username = response.name.trim();
    if username.is_empty() {
        return Err(HuggingFaceError::InvalidResponse);
    }

    Ok(HuggingFaceIdentity {
        username: username.to_owned(),
        email: normalized(response.email),
        token_display_name: response
            .auth
            .and_then(|auth| auth.access_token)
            .and_then(|token| normalized(token.display_name)),
    })
}

fn normalized(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn transport_error(error: reqwest::Error) -> HuggingFaceError {
    if error.is_timeout() {
        HuggingFaceError::Timeout
    } else {
        HuggingFaceError::RequestFailed
    }
}

fn status_error(status: StatusCode) -> Option<HuggingFaceError> {
    if status.is_success() {
        return None;
    }

    Some(match status {
        StatusCode::UNAUTHORIZED => HuggingFaceError::Unauthorized,
        StatusCode::FORBIDDEN => HuggingFaceError::InsufficientPermissions,
        StatusCode::TOO_MANY_REQUESTS => HuggingFaceError::RateLimited,
        _ => HuggingFaceError::RequestFailed,
    })
}

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;
    use serde_json::json;

    use super::*;

    fn response(value: serde_json::Value) -> WhoamiResponse {
        serde_json::from_value(value).expect("valid whoami test response")
    }

    #[test]
    fn maps_a_valid_identity() {
        assert_eq!(
            map_identity(response(json!({
                "name": " hf-user ",
                "email": " user@example.com ",
                "auth": {
                    "accessToken": {
                        "displayName": " LumaForge "
                    }
                }
            }))),
            Ok(HuggingFaceIdentity {
                username: "hf-user".to_string(),
                email: Some("user@example.com".to_string()),
                token_display_name: Some("LumaForge".to_string()),
            })
        );
    }

    #[test]
    fn normalizes_blank_optional_fields() {
        assert_eq!(
            map_identity(response(json!({
                "name": "hf-user",
                "email": " ",
                "auth": {
                    "accessToken": {
                        "displayName": "\n"
                    }
                }
            }))),
            Ok(HuggingFaceIdentity {
                username: "hf-user".to_string(),
                email: None,
                token_display_name: None,
            })
        );
    }

    #[test]
    fn rejects_a_blank_username() {
        assert_eq!(
            map_identity(response(json!({ "name": " " }))),
            Err(HuggingFaceError::InvalidResponse)
        );
    }

    #[test]
    fn classifies_http_statuses() {
        assert_eq!(status_error(StatusCode::OK), None);
        assert_eq!(
            status_error(StatusCode::UNAUTHORIZED),
            Some(HuggingFaceError::Unauthorized)
        );
        assert_eq!(
            status_error(StatusCode::FORBIDDEN),
            Some(HuggingFaceError::InsufficientPermissions)
        );
        assert_eq!(
            status_error(StatusCode::TOO_MANY_REQUESTS),
            Some(HuggingFaceError::RateLimited)
        );
        assert_eq!(
            status_error(StatusCode::INTERNAL_SERVER_ERROR),
            Some(HuggingFaceError::RequestFailed)
        );
    }
}
