use std::time::Duration;

use reqwest::{header::HeaderMap, header::CONTENT_TYPE, StatusCode};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    domain::{
        workflow::WorkflowPreset,
        workspace::{
            WorkspaceProvisioningPhase, WorkspaceProvisioningProgress, WorkspaceProvisioningStatus,
        },
    },
    secrets::ProvisionerWorkerBearerToken,
};

const PROVISIONER_WORKER_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_WORKER_DIAGNOSTIC_BYTES: usize = 512;

#[derive(Debug, Clone)]
pub struct ProvisionerWorkerHttpGateway {
    http: reqwest::Client,
}

impl Default for ProvisionerWorkerHttpGateway {
    fn default() -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(PROVISIONER_WORKER_REQUEST_TIMEOUT)
                .build()
                .expect("Provisioner Worker HTTP client should build"),
        }
    }
}

impl ProvisionerWorkerHttpGateway {
    #[cfg(test)]
    fn new_for_test(timeout: Duration) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(timeout)
                .build()
                .expect("Provisioner Worker HTTP client should build"),
        }
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

#[derive(Debug, Clone, Serialize)]
pub struct ProvisionerWorkerStartRequest {
    pub job_id: String,
    pub workflow_preset: WorkflowPreset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionerWorkerStatus {
    pub status: ProvisionerWorkerJobStatus,
    pub phase: ProvisionerWorkerPhase,
    pub progress_percent: Option<u8>,
    pub diagnostic: Option<String>,
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
    InstallingRuntime,
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
    InvalidPayload { diagnostic: Option<String> },
    #[error("provisioner worker terminal failure")]
    TerminalFailure { diagnostic: Option<String> },
}

#[derive(Debug, Deserialize)]
struct ProvisionerWorkerStatusResponse {
    status: Option<String>,
    job_id: Option<String>,
    phase: Option<String>,
    progress_percent: Option<u8>,
    diagnostic: Option<String>,
    diagnostic_message: Option<String>,
    error: Option<ProvisionerWorkerErrorResponse>,
}

#[derive(Debug, Deserialize)]
struct ProvisionerWorkerErrorResponse {
    code: Option<String>,
    reason_code: Option<String>,
    message: Option<String>,
}

async fn parse_worker_response(
    response: reqwest::Response,
) -> Result<ProvisionerWorkerStatus, ProvisionerWorkerError> {
    let status = response.status();
    let is_worker_json = has_json_content_type(response.headers());
    if !status.is_success() {
        let diagnostic = if is_worker_json {
            response
                .json::<ProvisionerWorkerErrorResponse>()
                .await
                .ok()
                .and_then(diagnostic_from_worker_error)
        } else {
            None
        };
        return Err(worker_error_from_status(status, diagnostic, is_worker_json));
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

fn worker_error_from_status(
    status: StatusCode,
    diagnostic: Option<String>,
    is_worker_json: bool,
) -> ProvisionerWorkerError {
    match status {
        _ if !is_worker_json => ProvisionerWorkerError::Unreachable,
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => ProvisionerWorkerError::Unauthorized,
        StatusCode::CONFLICT => ProvisionerWorkerError::Conflict,
        status if status.is_server_error() => ProvisionerWorkerError::Unreachable,
        _ => ProvisionerWorkerError::InvalidPayload { diagnostic },
    }
}

fn success_payload_error(is_worker_json: bool) -> ProvisionerWorkerError {
    if is_worker_json {
        ProvisionerWorkerError::InvalidPayload { diagnostic: None }
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
        .ok_or(ProvisionerWorkerError::InvalidPayload { diagnostic: None })?;
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
        _ => return Err(ProvisionerWorkerError::InvalidPayload { diagnostic: None }),
    };
    let phase = phase_from_response(response.phase.as_deref(), &status)?;
    if response
        .progress_percent
        .is_some_and(|percent| percent > 100)
    {
        return Err(ProvisionerWorkerError::InvalidPayload { diagnostic: None });
    }

    let diagnostic = response
        .error
        .and_then(diagnostic_from_worker_error)
        .or(response.diagnostic_message)
        .or(response.diagnostic)
        .map(sanitize_diagnostic);
    if status == ProvisionerWorkerJobStatus::Failed {
        return Err(ProvisionerWorkerError::TerminalFailure { diagnostic });
    }

    Ok(ProvisionerWorkerStatus {
        status,
        phase,
        progress_percent: response.progress_percent,
        diagnostic,
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
        Some("installing_runtime" | "installing_comfyui") => {
            Ok(ProvisionerWorkerPhase::InstallingRuntime)
        }
        Some("installing_models" | "downloading_assets") => {
            Ok(ProvisionerWorkerPhase::InstallingModels)
        }
        Some("installing_custom_nodes") => Ok(ProvisionerWorkerPhase::InstallingCustomNodes),
        Some("writing_manifest" | "validating_environment") => {
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
                Err(ProvisionerWorkerError::InvalidPayload { diagnostic: None })
            }
        },
        Some(_) => Err(ProvisionerWorkerError::InvalidPayload { diagnostic: None }),
    }
}

pub fn progress_from_worker_status(
    status: &ProvisionerWorkerStatus,
) -> WorkspaceProvisioningProgress {
    WorkspaceProvisioningProgress {
        status: match status.status {
            ProvisionerWorkerJobStatus::Idle | ProvisionerWorkerJobStatus::Running => {
                WorkspaceProvisioningStatus::Running
            }
            ProvisionerWorkerJobStatus::Cancelling => WorkspaceProvisioningStatus::Cancelling,
            ProvisionerWorkerJobStatus::Cancelled => WorkspaceProvisioningStatus::Cancelling,
            ProvisionerWorkerJobStatus::Succeeded => WorkspaceProvisioningStatus::Running,
            ProvisionerWorkerJobStatus::Failed => WorkspaceProvisioningStatus::Failed,
        },
        phase: match status.phase {
            ProvisionerWorkerPhase::Idle
            | ProvisionerWorkerPhase::Starting
            | ProvisionerWorkerPhase::ResolvingWorkflow
            | ProvisionerWorkerPhase::InstallingRuntime
            | ProvisionerWorkerPhase::InstallingModels
            | ProvisionerWorkerPhase::InstallingCustomNodes
            | ProvisionerWorkerPhase::WritingManifest => {
                WorkspaceProvisioningPhase::PreparingEnvironment
            }
            ProvisionerWorkerPhase::Completed => {
                WorkspaceProvisioningPhase::CreatingEndpointTemplate
            }
            ProvisionerWorkerPhase::Cancelled => WorkspaceProvisioningPhase::CleaningUp,
            ProvisionerWorkerPhase::Failed => WorkspaceProvisioningPhase::Failed,
        },
        percent: status.progress_percent,
        failure: None,
    }
}

fn diagnostic_from_worker_error(error: ProvisionerWorkerErrorResponse) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(code) = non_blank(error.code) {
        parts.push(format!("code: {code}"));
    }
    if let Some(reason_code) = non_blank(error.reason_code) {
        parts.push(format!("reason_code: {reason_code}"));
    }
    if let Some(message) = non_blank(error.message) {
        parts.push(format!("message: {message}"));
    }

    (!parts.is_empty()).then(|| sanitize_diagnostic(parts.join("\n")))
}

fn non_blank(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn sanitize_diagnostic(diagnostic: String) -> String {
    diagnostic
        .chars()
        .filter(|character| !character.is_control() || *character == '\n')
        .collect::<String>()
        .lines()
        .take(8)
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .take(MAX_WORKER_DIAGNOSTIC_BYTES)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_authorized_success_status_to_progress() {
        let status = status_from_response(ProvisionerWorkerStatusResponse {
            status: Some("running".to_string()),
            phase: Some("installing_models".to_string()),
            progress_percent: Some(42),
            diagnostic: Some("Downloading model".to_string()),
            diagnostic_message: None,
            error: None,
            ..status_response_defaults()
        })
        .expect("status should map");

        let progress = progress_from_worker_status(&status);

        assert_eq!(progress.status, WorkspaceProvisioningStatus::Running);
        assert_eq!(
            progress.phase,
            WorkspaceProvisioningPhase::PreparingEnvironment
        );
        assert_eq!(progress.percent, Some(42));
    }

    #[test]
    fn maps_terminal_failure_without_raw_control_output() {
        let error = status_from_response(ProvisionerWorkerStatusResponse {
            status: Some("failed".to_string()),
            phase: Some("failed".to_string()),
            progress_percent: Some(10),
            diagnostic: Some("failed\u{0} with token rp_secret".to_string()),
            diagnostic_message: None,
            error: None,
            ..status_response_defaults()
        })
        .expect_err("terminal failure should map");

        assert_eq!(
            error,
            ProvisionerWorkerError::TerminalFailure {
                diagnostic: Some("failed with token rp_secret".to_string()),
            }
        );
    }

    #[test]
    fn accepts_idle_status_without_phase() {
        let status = status_from_response(ProvisionerWorkerStatusResponse {
            status: Some("idle".to_string()),
            phase: None,
            ..status_response_defaults()
        })
        .expect("idle status without phase should map");

        assert_eq!(status.status, ProvisionerWorkerJobStatus::Idle);
        assert_eq!(status.phase, ProvisionerWorkerPhase::Idle);
    }

    #[test]
    fn accepts_terminal_success_without_phase() {
        let status = status_from_response(ProvisionerWorkerStatusResponse {
            status: Some("succeeded".to_string()),
            phase: None,
            progress_percent: Some(100),
            ..status_response_defaults()
        })
        .expect("terminal success without phase should map");

        assert_eq!(status.status, ProvisionerWorkerJobStatus::Succeeded);
        assert_eq!(status.phase, ProvisionerWorkerPhase::Completed);
    }

    #[test]
    fn maps_current_worker_phase_vocabulary() {
        for (worker_phase, expected_phase) in [
            ("starting", ProvisionerWorkerPhase::Starting),
            (
                "installing_comfyui",
                ProvisionerWorkerPhase::InstallingRuntime,
            ),
            (
                "installing_custom_nodes",
                ProvisionerWorkerPhase::InstallingCustomNodes,
            ),
            (
                "downloading_assets",
                ProvisionerWorkerPhase::InstallingModels,
            ),
            (
                "validating_environment",
                ProvisionerWorkerPhase::WritingManifest,
            ),
        ] {
            let status = status_from_response(ProvisionerWorkerStatusResponse {
                status: Some("running".to_string()),
                phase: Some(worker_phase.to_string()),
                ..status_response_defaults()
            })
            .expect("current worker phase should map");

            assert_eq!(status.phase, expected_phase);
            assert_eq!(
                progress_from_worker_status(&status).phase,
                WorkspaceProvisioningPhase::PreparingEnvironment
            );
        }
    }

    #[test]
    fn terminal_failure_uses_structured_worker_error_diagnostic() {
        let error = status_from_response(ProvisionerWorkerStatusResponse {
            status: Some("failed".to_string()),
            phase: None,
            diagnostic_message: Some("Download failed".to_string()),
            error: Some(ProvisionerWorkerErrorResponse {
                code: Some("asset_download_failed".to_string()),
                reason_code: Some("not_found".to_string()),
                message: Some("Model asset was not found".to_string()),
            }),
            ..status_response_defaults()
        })
        .expect_err("terminal failure should map");

        assert_eq!(
            error,
            ProvisionerWorkerError::TerminalFailure {
                diagnostic: Some(
                    "code: asset_download_failed\nreason_code: not_found\nmessage: Model asset was not found"
                        .to_string()
                ),
            }
        );
    }

    #[test]
    fn rejects_invalid_payloads() {
        let error = status_from_response(ProvisionerWorkerStatusResponse {
            status: Some("running".to_string()),
            phase: Some("installing_models".to_string()),
            progress_percent: Some(101),
            diagnostic: None,
            diagnostic_message: None,
            error: None,
            ..status_response_defaults()
        })
        .expect_err("invalid percentage should fail");

        assert_eq!(
            error,
            ProvisionerWorkerError::InvalidPayload { diagnostic: None }
        );
    }

    #[test]
    fn maps_unauthorized_and_conflict_statuses() {
        assert_eq!(
            worker_error_from_status(StatusCode::UNAUTHORIZED, None, true),
            ProvisionerWorkerError::Unauthorized
        );
        assert_eq!(
            worker_error_from_status(StatusCode::CONFLICT, None, true),
            ProvisionerWorkerError::Conflict
        );
    }

    #[test]
    fn maps_non_json_proxy_readiness_responses_to_unreachable() {
        for status in [
            StatusCode::UNAUTHORIZED,
            StatusCode::CONFLICT,
            StatusCode::NOT_FOUND,
        ] {
            assert_eq!(
                worker_error_from_status(status, None, false),
                ProvisionerWorkerError::Unreachable
            );
        }
        assert_eq!(
            success_payload_error(false),
            ProvisionerWorkerError::Unreachable
        );
    }

    #[test]
    fn maps_worker_json_contract_errors_to_invalid_payload_with_diagnostic() {
        assert_eq!(
            worker_error_from_status(
                StatusCode::BAD_REQUEST,
                Some("code: invalid_request".to_string()),
                true,
            ),
            ProvisionerWorkerError::InvalidPayload {
                diagnostic: Some("code: invalid_request".to_string()),
            }
        );
        assert_eq!(
            success_payload_error(true),
            ProvisionerWorkerError::InvalidPayload { diagnostic: None }
        );
    }

    #[tokio::test]
    async fn unreachable_worker_maps_to_unreachable() {
        let client = ProvisionerWorkerHttpGateway::new_for_test(Duration::from_millis(50));
        let token = ProvisionerWorkerBearerToken::new("worker-token".to_string()).expect("token");

        let error = tokio::time::timeout(
            Duration::from_secs(2),
            client.status("http://127.0.0.1:9/status", &token),
        )
        .await
        .expect("request should be bounded")
        .expect_err("unreachable worker should fail");

        assert_eq!(error, ProvisionerWorkerError::Unreachable);
    }

    fn status_response_defaults() -> ProvisionerWorkerStatusResponse {
        ProvisionerWorkerStatusResponse {
            status: None,
            job_id: None,
            phase: None,
            progress_percent: None,
            diagnostic: None,
            diagnostic_message: None,
            error: None,
        }
    }
}
