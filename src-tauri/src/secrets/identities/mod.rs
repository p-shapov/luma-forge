pub mod hugging_face;
pub mod runpod;

use std::time::Duration;

use crate::secrets::errors::{identity_request_error, SecretsStorageError};

pub(super) fn identity_http_client(
    connect_timeout: Duration,
    request_timeout: Duration,
) -> Result<reqwest::Client, SecretsStorageError> {
    reqwest::Client::builder()
        .connect_timeout(connect_timeout)
        .timeout(request_timeout)
        .build()
        .map_err(identity_request_error)
}
