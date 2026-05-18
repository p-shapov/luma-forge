use std::{future::Future, pin::Pin, time::Duration};

use reqwest::{header::HeaderMap, header::CONTENT_TYPE, StatusCode};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    domain::{
        runtime::ResolvedRuntimeImageSnapshot,
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
            | ProvisionerWorkerPhase::ValidatingRuntime
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
    use crate::domain::workspace::{WorkspaceProvisioningPhase, WorkspaceProvisioningStatus};

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
            diagnostic: None,
            diagnostic_message: None,
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

        assert_eq!(
            error,
            ProvisionerWorkerError::InvalidPayload { diagnostic: None }
        );
    }

    #[test]
    fn status_from_response_rejects_unsafe_progress_percent() {
        let error = status_from_response(response(
            Some("running"),
            Some("installing_models"),
            Some(101),
        ))
        .expect_err("progress above 100 should be invalid");

        assert_eq!(
            error,
            ProvisionerWorkerError::InvalidPayload { diagnostic: None }
        );
    }

    #[test]
    fn status_from_response_maps_failed_payload_to_terminal_failure_with_sanitized_diagnostic() {
        let mut payload = response(Some("failed"), Some("failed"), None);
        payload.diagnostic =
            Some("line1\u{0}\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9".to_string());

        let error = status_from_response(payload)
            .expect_err("failed status should become terminal worker failure");

        assert_eq!(
            error,
            ProvisionerWorkerError::TerminalFailure {
                diagnostic: Some(
                    "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8".to_string()
                ),
            }
        );
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
    fn progress_from_worker_status_maps_worker_status_to_workspace_progress() {
        let running = progress_from_worker_status(&ProvisionerWorkerStatus {
            status: ProvisionerWorkerJobStatus::Running,
            phase: ProvisionerWorkerPhase::InstallingCustomNodes,
            progress_percent: Some(55),
            diagnostic: None,
        });
        assert_eq!(running.status, WorkspaceProvisioningStatus::Running);
        assert_eq!(
            running.phase,
            WorkspaceProvisioningPhase::PreparingEnvironment
        );
        assert_eq!(running.percent, Some(55));
        assert_eq!(running.failure, None);

        let cancelling = progress_from_worker_status(&ProvisionerWorkerStatus {
            status: ProvisionerWorkerJobStatus::Cancelling,
            phase: ProvisionerWorkerPhase::Cancelled,
            progress_percent: None,
            diagnostic: None,
        });
        assert_eq!(cancelling.status, WorkspaceProvisioningStatus::Cancelling);
        assert_eq!(cancelling.phase, WorkspaceProvisioningPhase::CleaningUp);

        let completed = progress_from_worker_status(&ProvisionerWorkerStatus {
            status: ProvisionerWorkerJobStatus::Succeeded,
            phase: ProvisionerWorkerPhase::Completed,
            progress_percent: Some(100),
            diagnostic: None,
        });
        assert_eq!(completed.status, WorkspaceProvisioningStatus::Running);
        assert_eq!(
            completed.phase,
            WorkspaceProvisioningPhase::CreatingEndpointTemplate
        );
    }

    #[test]
    fn worker_error_from_status_classifies_http_failures() {
        assert_eq!(
            worker_error_from_status(StatusCode::UNAUTHORIZED, None, true),
            ProvisionerWorkerError::Unauthorized
        );
        assert_eq!(
            worker_error_from_status(StatusCode::FORBIDDEN, None, true),
            ProvisionerWorkerError::Unauthorized
        );
        assert_eq!(
            worker_error_from_status(StatusCode::CONFLICT, None, true),
            ProvisionerWorkerError::Conflict
        );
        assert_eq!(
            worker_error_from_status(StatusCode::BAD_GATEWAY, None, true),
            ProvisionerWorkerError::Unreachable
        );
        assert_eq!(
            worker_error_from_status(StatusCode::BAD_REQUEST, Some("bad".to_string()), true),
            ProvisionerWorkerError::InvalidPayload {
                diagnostic: Some("bad".to_string())
            }
        );
        assert_eq!(
            worker_error_from_status(StatusCode::BAD_REQUEST, None, false),
            ProvisionerWorkerError::Unreachable
        );
    }
}
