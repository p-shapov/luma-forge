use reqwest::StatusCode;
use serde::Deserialize;

use crate::{
    domain::provisioned_remote::{ProvisionedRemoteProvisionerStatus, RunpodLifecycleError},
    shared::AppFuture,
};

pub trait ProvisionerWorkerApi: Send + Sync {
    fn get_status<'a>(
        &'a self,
        status_url: &'a str,
        bearer_token: &'a str,
    ) -> AppFuture<'a, Result<ProvisionedRemoteProvisionerStatus, RunpodLifecycleError>>;
}

#[derive(Clone)]
pub struct ProvisionerWorkerClient {
    http: reqwest::Client,
}

impl ProvisionerWorkerClient {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }
}

impl ProvisionerWorkerApi for ProvisionerWorkerClient {
    fn get_status<'a>(
        &'a self,
        status_url: &'a str,
        bearer_token: &'a str,
    ) -> AppFuture<'a, Result<ProvisionedRemoteProvisionerStatus, RunpodLifecycleError>> {
        Box::pin(async move {
            let response = self
                .http
                .get(status_url)
                .bearer_auth(bearer_token)
                .send()
                .await
                .map_err(|_| RunpodLifecycleError::ProvisionerUnavailable)?;

            map_http_status(response.status())?;
            let status = response
                .json::<ProvisionerStatusResponse>()
                .await
                .map_err(|_| RunpodLifecycleError::ProvisionerResponseInvalid)?;

            map_status_response(status)
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct ProvisionerStatusResponse {
    status: String,
    error: Option<ProvisionerWorkerErrorResponse>,
}

#[derive(Debug, Deserialize)]
pub struct ProvisionerWorkerErrorResponse {
    #[serde(alias = "code")]
    _code: String,
    #[serde(alias = "message")]
    _message: String,
}

pub fn map_status_response(
    response: ProvisionerStatusResponse,
) -> Result<ProvisionedRemoteProvisionerStatus, RunpodLifecycleError> {
    match response.status.as_str() {
        "idle" => Ok(ProvisionedRemoteProvisionerStatus::Pending),
        "running" => Ok(ProvisionedRemoteProvisionerStatus::Running),
        "succeeded" => Ok(ProvisionedRemoteProvisionerStatus::Succeeded),
        "failed" => {
            let _error = response
                .error
                .ok_or(RunpodLifecycleError::ProvisionerResponseInvalid)?;
            Ok(ProvisionedRemoteProvisionerStatus::Failed)
        }
        _ => Err(RunpodLifecycleError::ProvisionerResponseInvalid),
    }
}

fn map_http_status(status: StatusCode) -> Result<(), RunpodLifecycleError> {
    match status {
        status if status.is_success() => Ok(()),
        StatusCode::UNAUTHORIZED => Err(RunpodLifecycleError::ProvisionerResponseInvalid),
        StatusCode::CONFLICT => Err(RunpodLifecycleError::ProvisionerFailed),
        _ => Err(RunpodLifecycleError::ProvisionerUnavailable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_status_response_maps_lifecycle_statuses() {
        assert_eq!(
            map_status_response(ProvisionerStatusResponse {
                status: "idle".to_string(),
                error: None,
            }),
            Ok(ProvisionedRemoteProvisionerStatus::Pending)
        );
        assert_eq!(
            map_status_response(ProvisionerStatusResponse {
                status: "running".to_string(),
                error: None,
            }),
            Ok(ProvisionedRemoteProvisionerStatus::Running)
        );
        assert_eq!(
            map_status_response(ProvisionerStatusResponse {
                status: "succeeded".to_string(),
                error: None,
            }),
            Ok(ProvisionedRemoteProvisionerStatus::Succeeded)
        );
    }

    #[test]
    fn map_status_response_maps_worker_failure_details() {
        assert_eq!(
            map_status_response(ProvisionerStatusResponse {
                status: "failed".to_string(),
                error: Some(ProvisionerWorkerErrorResponse {
                    _code: "asset_download_failed".to_string(),
                    _message: "download failed".to_string(),
                }),
            }),
            Ok(ProvisionedRemoteProvisionerStatus::Failed)
        );
    }

    #[test]
    fn map_status_response_rejects_malformed_responses() {
        assert_eq!(
            map_status_response(ProvisionerStatusResponse {
                status: "failed".to_string(),
                error: None,
            }),
            Err(RunpodLifecycleError::ProvisionerResponseInvalid)
        );
        assert_eq!(
            map_status_response(ProvisionerStatusResponse {
                status: "other".to_string(),
                error: None,
            }),
            Err(RunpodLifecycleError::ProvisionerResponseInvalid)
        );
    }

    #[test]
    fn map_http_status_maps_worker_errors() {
        assert_eq!(map_http_status(StatusCode::OK), Ok(()));
        assert_eq!(
            map_http_status(StatusCode::UNAUTHORIZED),
            Err(RunpodLifecycleError::ProvisionerResponseInvalid)
        );
        assert_eq!(
            map_http_status(StatusCode::CONFLICT),
            Err(RunpodLifecycleError::ProvisionerFailed)
        );
        assert_eq!(
            map_http_status(StatusCode::INTERNAL_SERVER_ERROR),
            Err(RunpodLifecycleError::ProvisionerUnavailable)
        );
    }
}
