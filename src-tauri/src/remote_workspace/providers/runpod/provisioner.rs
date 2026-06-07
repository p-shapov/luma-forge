use reqwest::StatusCode;
use serde::Deserialize;

use crate::{
    domain::workspace::{RemoteProvisionerStatus, RemoteProvisioningError},
    shared::AppFuture,
};

pub trait ProvisionerWorkerApi: Send + Sync {
    fn get_status<'a>(
        &'a self,
        status_url: &'a str,
        bearer_token: &'a str,
    ) -> AppFuture<'a, Result<RemoteProvisionerStatus, RemoteProvisioningError>>;
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
    ) -> AppFuture<'a, Result<RemoteProvisionerStatus, RemoteProvisioningError>> {
        Box::pin(async move {
            let response = self
                .http
                .get(status_url)
                .bearer_auth(bearer_token)
                .send()
                .await
                .map_err(|_| RemoteProvisioningError::ProvisionerWorkerUnavailable)?;

            map_http_status(response.status())?;
            let status = response
                .json::<ProvisionerStatusResponse>()
                .await
                .map_err(|_| RemoteProvisioningError::ProvisionerWorkerResponseInvalid)?;

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
    code: String,
    message: String,
}

pub fn map_status_response(
    response: ProvisionerStatusResponse,
) -> Result<RemoteProvisionerStatus, RemoteProvisioningError> {
    match response.status.as_str() {
        "idle" => Ok(RemoteProvisionerStatus::Pending),
        "running" => Ok(RemoteProvisionerStatus::Running),
        "succeeded" => Ok(RemoteProvisionerStatus::Succeeded),
        "failed" => {
            let error = response
                .error
                .ok_or(RemoteProvisioningError::ProvisionerWorkerResponseInvalid)?;
            Ok(RemoteProvisionerStatus::Failed {
                code: error.code,
                message: error.message,
            })
        }
        _ => Err(RemoteProvisioningError::ProvisionerWorkerResponseInvalid),
    }
}

fn map_http_status(status: StatusCode) -> Result<(), RemoteProvisioningError> {
    match status {
        status if status.is_success() => Ok(()),
        StatusCode::UNAUTHORIZED => Err(RemoteProvisioningError::ProvisionerWorkerUnauthorized),
        StatusCode::CONFLICT => Err(RemoteProvisioningError::ProvisionerWorkerConflict),
        _ => Err(RemoteProvisioningError::ProvisionerWorkerUnexpectedError),
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
            Ok(RemoteProvisionerStatus::Pending)
        );
        assert_eq!(
            map_status_response(ProvisionerStatusResponse {
                status: "running".to_string(),
                error: None,
            }),
            Ok(RemoteProvisionerStatus::Running)
        );
        assert_eq!(
            map_status_response(ProvisionerStatusResponse {
                status: "succeeded".to_string(),
                error: None,
            }),
            Ok(RemoteProvisionerStatus::Succeeded)
        );
    }

    #[test]
    fn map_status_response_maps_worker_failure_details() {
        assert_eq!(
            map_status_response(ProvisionerStatusResponse {
                status: "failed".to_string(),
                error: Some(ProvisionerWorkerErrorResponse {
                    code: "asset_download_failed".to_string(),
                    message: "download failed".to_string(),
                }),
            }),
            Ok(RemoteProvisionerStatus::Failed {
                code: "asset_download_failed".to_string(),
                message: "download failed".to_string(),
            })
        );
    }

    #[test]
    fn map_status_response_rejects_malformed_responses() {
        assert_eq!(
            map_status_response(ProvisionerStatusResponse {
                status: "failed".to_string(),
                error: None,
            }),
            Err(RemoteProvisioningError::ProvisionerWorkerResponseInvalid)
        );
        assert_eq!(
            map_status_response(ProvisionerStatusResponse {
                status: "other".to_string(),
                error: None,
            }),
            Err(RemoteProvisioningError::ProvisionerWorkerResponseInvalid)
        );
    }

    #[test]
    fn map_http_status_maps_worker_errors() {
        assert_eq!(map_http_status(StatusCode::OK), Ok(()));
        assert_eq!(
            map_http_status(StatusCode::UNAUTHORIZED),
            Err(RemoteProvisioningError::ProvisionerWorkerUnauthorized)
        );
        assert_eq!(
            map_http_status(StatusCode::CONFLICT),
            Err(RemoteProvisioningError::ProvisionerWorkerConflict)
        );
        assert_eq!(
            map_http_status(StatusCode::INTERNAL_SERVER_ERROR),
            Err(RemoteProvisioningError::ProvisionerWorkerUnexpectedError)
        );
    }
}
