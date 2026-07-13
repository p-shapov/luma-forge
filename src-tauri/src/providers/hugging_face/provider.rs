use secrecy::ExposeSecret;

use crate::providers::{http, http::ResponseExt, NetworkError};

use super::{generated::WhoamiResponse, IdentityRequest, IdentityResponse};

const WHOAMI_URL: &str = "https://huggingface.co/api/whoami-v2";

#[derive(Clone)]
pub struct HuggingFaceProvider {
    http: reqwest::Client,
}

impl HuggingFaceProvider {
    pub fn new() -> Result<Self, NetworkError> {
        Ok(Self {
            http: http::client()?,
        })
    }

    #[crate::diagnostics::diagnostic(show_output, show_error)]
    pub async fn identity(
        &self,
        #[diagnostic(show)] request: IdentityRequest,
    ) -> Result<IdentityResponse, NetworkError> {
        let response = self
            .http
            .get(WHOAMI_URL)
            .bearer_auth(request.credential.expose_secret())
            .send()
            .await
            .into_json::<WhoamiResponse>()
            .await?;
        Ok(identity_response(response))
    }
}

fn identity_response(response: WhoamiResponse) -> IdentityResponse {
    IdentityResponse {
        key_name: response
            .auth
            .access_token
            .map(|access_token| access_token.display_name),
        username: response.name,
        email: response.email,
    }
}
