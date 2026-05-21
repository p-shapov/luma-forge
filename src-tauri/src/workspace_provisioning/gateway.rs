use std::{future::Future, pin::Pin, time::Duration};

use reqwest::{header::HeaderMap, header::CONTENT_TYPE, StatusCode};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    domain::{runtime::ResolvedRuntimeImageSnapshot, workflow::WorkflowPreset},
    secrets::ProvisionerWorkerBearerToken,
};

const PROVISIONER_WORKER_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone)]
pub struct ProvisionerWorkerHttpGateway {
    http: reqwest::Client,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("Provisioner Worker HTTP gateway initialization failed")]
pub struct ProvisionerWorkerHttpGatewayInitError;

impl ProvisionerWorkerHttpGateway {
    pub fn try_new() -> Result<Self, ProvisionerWorkerHttpGatewayInitError> {
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(PROVISIONER_WORKER_REQUEST_TIMEOUT)
                .build()
                .map_err(|_| ProvisionerWorkerHttpGatewayInitError)?,
        })
    }

    pub async fn start(
        &self,
        provisioner_status_url: &str,
        token: &ProvisionerWorkerBearerToken,
        request: &ProvisionerWorkerStartRequest,
    ) -> Result<ProvisionerWorkerStatus, ProvisionerWorkerError> {
        let response = self
            .http
            .post(worker_url(provisioner_status_url, "start")?)
            .bearer_auth(token.expose_secret())
            .json(request)
            .send()
            .await
            .map_err(|_| ProvisionerWorkerError::Unreachable)?;
        parse_worker_response(response).await
    }

    pub async fn status(
        &self,
        provisioner_status_url: &str,
        token: &ProvisionerWorkerBearerToken,
    ) -> Result<ProvisionerWorkerStatus, ProvisionerWorkerError> {
        let response = self
            .http
            .get(provisioner_status_url)
            .bearer_auth(token.expose_secret())
            .send()
            .await
            .map_err(|_| ProvisionerWorkerError::Unreachable)?;
        parse_worker_response(response).await
    }
}

pub trait ProvisionerWorkerGateway: Send + Sync {
    fn start<'a>(
        &'a self,
        provisioner_status_url: &'a str,
        token: &'a ProvisionerWorkerBearerToken,
        request: &'a ProvisionerWorkerStartRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisionerWorkerStatus, ProvisionerWorkerError>>
                + Send
                + 'a,
        >,
    >;

    fn status<'a>(
        &'a self,
        provisioner_status_url: &'a str,
        token: &'a ProvisionerWorkerBearerToken,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisionerWorkerStatus, ProvisionerWorkerError>>
                + Send
                + 'a,
        >,
    >;
}

impl ProvisionerWorkerGateway for ProvisionerWorkerHttpGateway {
    fn start<'a>(
        &'a self,
        provisioner_status_url: &'a str,
        token: &'a ProvisionerWorkerBearerToken,
        request: &'a ProvisionerWorkerStartRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisionerWorkerStatus, ProvisionerWorkerError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            ProvisionerWorkerHttpGateway::start(self, provisioner_status_url, token, request).await
        })
    }

    fn status<'a>(
        &'a self,
        provisioner_status_url: &'a str,
        token: &'a ProvisionerWorkerBearerToken,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisionerWorkerStatus, ProvisionerWorkerError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            ProvisionerWorkerHttpGateway::status(self, provisioner_status_url, token).await
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProvisionerWorkerStartRequest {
    pub job_id: String,
    pub workflow_preset: WorkflowPreset,
    pub resolved_runtime_image: ResolvedRuntimeImageSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionerWorkerStatus {
    pub status: ProvisionerWorkerJobStatus,
    pub phase: ProvisionerWorkerPhase,
    pub progress_percent: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvisionerWorkerJobStatus {
    Idle,
    Running,
    Cancelling,
    Cancelled,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvisionerWorkerPhase {
    Idle,
    Starting,
    ResolvingWorkflow,
    ValidatingRuntime,
    InstallingModels,
    InstallingCustomNodes,
    WritingManifest,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ProvisionerWorkerError {
    #[error("provisioner worker unauthorized")]
    Unauthorized,
    #[error("provisioner worker conflict")]
    Conflict,
    #[error("provisioner worker unreachable")]
    Unreachable,
    #[error("provisioner worker payload invalid")]
    InvalidPayload,
    #[error("provisioner worker failed")]
    Failed,
    #[error("provisioner worker git checkout failed")]
    GitCheckoutFailed,
    #[error("provisioner worker dependency install failed")]
    DependencyInstallFailed,
    #[error("provisioner worker asset download failed")]
    AssetDownloadFailed,
    #[error("provisioner worker asset auth required")]
    AssetAuthRequired,
    #[error("provisioner worker path validation failed")]
    PathValidationFailed,
    #[error("provisioner worker step timeout")]
    StepTimeout,
    #[error("provisioner worker unexpected error")]
    UnexpectedError,
}

#[derive(Debug, Deserialize)]
struct ProvisionerWorkerStatusResponse {
    status: Option<String>,
    job_id: Option<String>,
    phase: Option<String>,
    progress_percent: Option<u8>,
    error: Option<ProvisionerWorkerErrorResponse>,
}

#[derive(Debug, Deserialize)]
struct ProvisionerWorkerErrorResponse {
    code: Option<String>,
    reason_code: Option<String>,
}

async fn parse_worker_response(
    response: reqwest::Response,
) -> Result<ProvisionerWorkerStatus, ProvisionerWorkerError> {
    let status = response.status();
    let is_worker_json = has_json_content_type(response.headers());
    if !status.is_success() {
        return Err(worker_error_from_status(status, is_worker_json));
    }

    if !is_worker_json {
        return Err(success_payload_error(is_worker_json));
    }

    let payload = response
        .json::<ProvisionerWorkerStatusResponse>()
        .await
        .map_err(|_| success_payload_error(is_worker_json))?;
    status_from_response(payload)
}

fn worker_error_from_status(status: StatusCode, is_worker_json: bool) -> ProvisionerWorkerError {
    match status {
        _ if !is_worker_json => ProvisionerWorkerError::Unreachable,
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => ProvisionerWorkerError::Unauthorized,
        StatusCode::CONFLICT => ProvisionerWorkerError::Conflict,
        status if status.is_server_error() => ProvisionerWorkerError::Unreachable,
        _ => ProvisionerWorkerError::InvalidPayload,
    }
}

fn success_payload_error(is_worker_json: bool) -> ProvisionerWorkerError {
    if is_worker_json {
        ProvisionerWorkerError::InvalidPayload
    } else {
        ProvisionerWorkerError::Unreachable
    }
}

fn has_json_content_type(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value.split(';').next().is_some_and(|media_type| {
                media_type.trim().eq_ignore_ascii_case("application/json")
            })
        })
}

fn worker_url(provisioner_status_url: &str, path: &str) -> Result<String, ProvisionerWorkerError> {
    let base_url = provisioner_status_url
        .strip_suffix("/status")
        .ok_or(ProvisionerWorkerError::InvalidPayload)?;
    Ok(format!("{base_url}/{path}"))
}

fn status_from_response(
    response: ProvisionerWorkerStatusResponse,
) -> Result<ProvisionerWorkerStatus, ProvisionerWorkerError> {
    let _job_id = response.job_id.as_deref();
    let status = match response.status.as_deref() {
        Some("idle") => ProvisionerWorkerJobStatus::Idle,
        Some("running") => ProvisionerWorkerJobStatus::Running,
        Some("cancelling") => ProvisionerWorkerJobStatus::Cancelling,
        Some("cancelled") => ProvisionerWorkerJobStatus::Cancelled,
        Some("succeeded") => ProvisionerWorkerJobStatus::Succeeded,
        Some("failed") => ProvisionerWorkerJobStatus::Failed,
        _ => return Err(ProvisionerWorkerError::InvalidPayload),
    };
    let phase = phase_from_response(response.phase.as_deref(), &status)?;
    if response
        .progress_percent
        .is_some_and(|percent| percent > 100)
    {
        return Err(ProvisionerWorkerError::InvalidPayload);
    }

    let failure = response
        .error
        .map(terminal_failure_from_worker_error)
        .unwrap_or(ProvisionerWorkerError::Failed);
    if status == ProvisionerWorkerJobStatus::Failed {
        return Err(failure);
    }

    Ok(ProvisionerWorkerStatus {
        status,
        phase,
        progress_percent: response.progress_percent,
    })
}

fn phase_from_response(
    phase: Option<&str>,
    status: &ProvisionerWorkerJobStatus,
) -> Result<ProvisionerWorkerPhase, ProvisionerWorkerError> {
    match phase {
        Some("idle") => Ok(ProvisionerWorkerPhase::Idle),
        Some("starting") => Ok(ProvisionerWorkerPhase::Starting),
        Some("resolving_workflow") => Ok(ProvisionerWorkerPhase::ResolvingWorkflow),
        Some("materializing_runtime" | "installing_runtime" | "installing_comfyui") => {
            Ok(ProvisionerWorkerPhase::ValidatingRuntime)
        }
        Some("installing_models" | "downloading_assets") => {
            Ok(ProvisionerWorkerPhase::InstallingModels)
        }
        Some("preparing_custom_nodes" | "installing_custom_nodes") => {
            Ok(ProvisionerWorkerPhase::InstallingCustomNodes)
        }
        Some("writing_manifest" | "validating_environment" | "verifying_assets") => {
            Ok(ProvisionerWorkerPhase::WritingManifest)
        }
        Some("completed") => Ok(ProvisionerWorkerPhase::Completed),
        Some("cancelled") => Ok(ProvisionerWorkerPhase::Cancelled),
        Some("failed") => Ok(ProvisionerWorkerPhase::Failed),
        None => match status {
            ProvisionerWorkerJobStatus::Idle => Ok(ProvisionerWorkerPhase::Idle),
            ProvisionerWorkerJobStatus::Succeeded => Ok(ProvisionerWorkerPhase::Completed),
            ProvisionerWorkerJobStatus::Cancelled => Ok(ProvisionerWorkerPhase::Cancelled),
            ProvisionerWorkerJobStatus::Failed => Ok(ProvisionerWorkerPhase::Failed),
            ProvisionerWorkerJobStatus::Running | ProvisionerWorkerJobStatus::Cancelling => {
                Err(ProvisionerWorkerError::InvalidPayload)
            }
        },
        Some(_) => Err(ProvisionerWorkerError::InvalidPayload),
    }
}

fn terminal_failure_from_worker_error(
    error: ProvisionerWorkerErrorResponse,
) -> ProvisionerWorkerError {
    provisioner_worker_failure_code(error.code.as_deref(), error.reason_code.as_deref())
}

fn provisioner_worker_failure_code(
    code: Option<&str>,
    reason_code: Option<&str>,
) -> ProvisionerWorkerError {
    reason_code
        .and_then(known_provisioner_worker_failure_code)
        .or_else(|| code.and_then(known_provisioner_worker_failure_code))
        .unwrap_or(ProvisionerWorkerError::Failed)
}

fn known_provisioner_worker_failure_code(value: &str) -> Option<ProvisionerWorkerError> {
    match value {
        "git_checkout_failed" => ProvisionerWorkerError::GitCheckoutFailed,
        "dependency_install_failed" => ProvisionerWorkerError::DependencyInstallFailed,
        "asset_download_failed" => ProvisionerWorkerError::AssetDownloadFailed,
        "asset_auth_required" => ProvisionerWorkerError::AssetAuthRequired,
        "path_validation_failed" => ProvisionerWorkerError::PathValidationFailed,
        "step_timeout" => ProvisionerWorkerError::StepTimeout,
        "unexpected_exception" | "unexpected_error" => ProvisionerWorkerError::UnexpectedError,
        _ => return None,
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(
        status: Option<&str>,
        phase: Option<&str>,
        progress_percent: Option<u8>,
    ) -> ProvisionerWorkerStatusResponse {
        ProvisionerWorkerStatusResponse {
            status: status.map(str::to_string),
            job_id: Some("workspace-1".to_string()),
            phase: phase.map(str::to_string),
            progress_percent,
            error: None,
        }
    }

    #[test]
    fn status_from_response_accepts_idle_and_succeeded_without_phase() {
        let idle = status_from_response(response(Some("idle"), None, None))
            .expect("idle with no phase should be valid");
        assert_eq!(idle.status, ProvisionerWorkerJobStatus::Idle);
        assert_eq!(idle.phase, ProvisionerWorkerPhase::Idle);

        let succeeded = status_from_response(response(Some("succeeded"), None, Some(100)))
            .expect("succeeded with no phase should be valid");
        assert_eq!(succeeded.status, ProvisionerWorkerJobStatus::Succeeded);
        assert_eq!(succeeded.phase, ProvisionerWorkerPhase::Completed);
        assert_eq!(succeeded.progress_percent, Some(100));
    }

    #[test]
    fn status_from_response_rejects_running_without_phase() {
        let error = status_from_response(response(Some("running"), None, Some(10)))
            .expect_err("running without phase should be invalid");

        assert_eq!(error, ProvisionerWorkerError::InvalidPayload);
    }

    #[test]
    fn status_from_response_rejects_unsafe_progress_percent() {
        let error = status_from_response(response(
            Some("running"),
            Some("installing_models"),
            Some(101),
        ))
        .expect_err("progress above 100 should be invalid");

        assert_eq!(error, ProvisionerWorkerError::InvalidPayload);
    }

    #[test]
    fn status_from_response_maps_failed_payload_to_terminal_failure_code() {
        let mut payload = response(Some("failed"), Some("failed"), None);
        payload.error = Some(ProvisionerWorkerErrorResponse {
            code: Some("dependency_install_failed".to_string()),
            reason_code: Some("dependency_install_failed".to_string()),
        });

        let error = status_from_response(payload)
            .expect_err("failed status should become terminal worker failure");

        assert_eq!(error, ProvisionerWorkerError::DependencyInstallFailed);
    }

    #[test]
    fn worker_error_reason_code_maps_to_terminal_failure_code() {
        let code = terminal_failure_from_worker_error(ProvisionerWorkerErrorResponse {
            code: Some("dependency_install_failed".to_string()),
            reason_code: Some("dependency_install_failed".to_string()),
        });

        assert_eq!(code, ProvisionerWorkerError::DependencyInstallFailed);
    }

    #[test]
    fn worker_error_unknown_reason_code_falls_back_to_recognized_code() {
        let code = terminal_failure_from_worker_error(ProvisionerWorkerErrorResponse {
            code: Some("dependency_install_failed".to_string()),
            reason_code: Some("future_worker_reason".to_string()),
        });

        assert_eq!(code, ProvisionerWorkerError::DependencyInstallFailed);
    }

    #[test]
    fn status_from_response_normalizes_worker_phase_aliases() {
        let runtime = status_from_response(response(
            Some("running"),
            Some("installing_comfyui"),
            Some(20),
        ))
        .expect("runtime phase alias should be valid");
        assert_eq!(runtime.phase, ProvisionerWorkerPhase::ValidatingRuntime);

        let assets = status_from_response(response(
            Some("running"),
            Some("downloading_assets"),
            Some(60),
        ))
        .expect("asset phase alias should be valid");
        assert_eq!(assets.phase, ProvisionerWorkerPhase::InstallingModels);

        let manifest = status_from_response(response(
            Some("running"),
            Some("verifying_assets"),
            Some(90),
        ))
        .expect("manifest phase alias should be valid");
        assert_eq!(manifest.phase, ProvisionerWorkerPhase::WritingManifest);
    }

    #[test]
    fn worker_error_from_status_classifies_http_failures() {
        assert_eq!(
            worker_error_from_status(StatusCode::UNAUTHORIZED, true),
            ProvisionerWorkerError::Unauthorized
        );
        assert_eq!(
            worker_error_from_status(StatusCode::FORBIDDEN, true),
            ProvisionerWorkerError::Unauthorized
        );
        assert_eq!(
            worker_error_from_status(StatusCode::CONFLICT, true),
            ProvisionerWorkerError::Conflict
        );
        assert_eq!(
            worker_error_from_status(StatusCode::BAD_GATEWAY, true),
            ProvisionerWorkerError::Unreachable
        );
        assert_eq!(
            worker_error_from_status(StatusCode::BAD_REQUEST, true),
            ProvisionerWorkerError::InvalidPayload
        );
        assert_eq!(
            worker_error_from_status(StatusCode::BAD_REQUEST, false),
            ProvisionerWorkerError::Unreachable
        );
    }
}
