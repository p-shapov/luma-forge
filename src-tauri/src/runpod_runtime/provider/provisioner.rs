use reqwest::StatusCode;
use serde::Deserialize;

use crate::{runpod_runtime::errors::RunpodRuntimeError, shared::AppFuture};

use super::RunpodProvisionerStatus;

const STATUS_IDLE: &str = "idle";
const STATUS_RUNNING: &str = "running";
const STATUS_SUCCEEDED: &str = "succeeded";
const STATUS_FAILED: &str = "failed";

pub trait ProvisionerWorkerApi: Send + Sync {
    fn get_status<'a>(
        &'a self,
        status_url: &'a str,
        bearer_token: &'a str,
    ) -> AppFuture<'a, Result<RunpodProvisionerStatus, RunpodRuntimeError>>;
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
    ) -> AppFuture<'a, Result<RunpodProvisionerStatus, RunpodRuntimeError>> {
        Box::pin(async move {
            let response = self
                .http
                .get(status_url)
                .bearer_auth(bearer_token)
                .send()
                .await
                .map_err(|_| provisioner_unavailable())?;

            map_http_status(response.status())?;
            let status = response
                .json::<ProvisionerStatusResponse>()
                .await
                .map_err(|_| provisioner_response_invalid())?;

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
) -> Result<RunpodProvisionerStatus, RunpodRuntimeError> {
    match response.status.as_str() {
        STATUS_IDLE => Ok(RunpodProvisionerStatus::Pending),
        STATUS_RUNNING => Ok(RunpodProvisionerStatus::Running),
        STATUS_SUCCEEDED => Ok(RunpodProvisionerStatus::Succeeded),
        STATUS_FAILED => {
            let _error = response.error.ok_or_else(provisioner_response_invalid)?;
            Ok(RunpodProvisionerStatus::Failed)
        }
        _ => Err(provisioner_response_invalid()),
    }
}

fn map_http_status(status: StatusCode) -> Result<(), RunpodRuntimeError> {
    match status {
        status if status.is_success() => Ok(()),
        StatusCode::UNAUTHORIZED => Err(provisioner_response_invalid()),
        StatusCode::CONFLICT => Err(provisioner_failed()),
        _ => Err(provisioner_unavailable()),
    }
}

fn provisioner_unavailable() -> RunpodRuntimeError {
    RunpodRuntimeError::ProvisionerWorkerUnavailable {
        message: "provisioner worker is unavailable".to_string(),
    }
}

fn provisioner_response_invalid() -> RunpodRuntimeError {
    RunpodRuntimeError::ProvisionerWorkerResponseInvalid {
        message: "provisioner worker response is invalid".to_string(),
    }
}

fn provisioner_failed() -> RunpodRuntimeError {
    RunpodRuntimeError::ProvisionerWorkerFailed {
        message: "provisioner worker failed".to_string(),
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
            Ok(RunpodProvisionerStatus::Pending)
        );
        assert_eq!(
            map_status_response(ProvisionerStatusResponse {
                status: "running".to_string(),
                error: None,
            }),
            Ok(RunpodProvisionerStatus::Running)
        );
        assert_eq!(
            map_status_response(ProvisionerStatusResponse {
                status: "succeeded".to_string(),
                error: None,
            }),
            Ok(RunpodProvisionerStatus::Succeeded)
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
            Ok(RunpodProvisionerStatus::Failed)
        );
    }

    #[test]
    fn map_status_response_rejects_malformed_responses() {
        assert_eq!(
            map_status_response(ProvisionerStatusResponse {
                status: "failed".to_string(),
                error: None,
            }),
            Err(provisioner_response_invalid())
        );
        assert_eq!(
            map_status_response(ProvisionerStatusResponse {
                status: "other".to_string(),
                error: None,
            }),
            Err(provisioner_response_invalid())
        );
    }

    #[test]
    fn map_http_status_maps_worker_errors() {
        assert_eq!(map_http_status(StatusCode::OK), Ok(()));
        assert_eq!(
            map_http_status(StatusCode::UNAUTHORIZED),
            Err(provisioner_response_invalid())
        );
        assert_eq!(
            map_http_status(StatusCode::CONFLICT),
            Err(provisioner_failed())
        );
        assert_eq!(
            map_http_status(StatusCode::INTERNAL_SERVER_ERROR),
            Err(provisioner_unavailable())
        );
    }
}
