use std::time::Duration;

use reqwest::StatusCode;
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

    pub async fn cancel(
        &self,
        provisioner_status_url: &str,
        token: &ProvisionerWorkerBearerToken,
    ) -> Result<ProvisionerWorkerStatus, ProvisionerWorkerError> {
        let response = self
            .http
            .post(worker_url(provisioner_status_url, "cancel")?)
            .bearer_auth(token.expose_secret())
            .send()
            .await
            .map_err(|_| ProvisionerWorkerError::Unreachable)?;
        parse_worker_response(response).await
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProvisionerWorkerStartRequest {
    pub workspace_id: String,
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
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvisionerWorkerPhase {
    Idle,
    ResolvingWorkflow,
    InstallingRuntime,
    InstallingModels,
    InstallingCustomNodes,
    WritingManifest,
    Completed,
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
    #[error("provisioner worker terminal failure")]
    TerminalFailure { diagnostic: Option<String> },
}

#[derive(Debug, Deserialize)]
struct ProvisionerWorkerStatusResponse {
    status: Option<String>,
    phase: Option<String>,
    progress_percent: Option<u8>,
    diagnostic: Option<String>,
}

async fn parse_worker_response(
    response: reqwest::Response,
) -> Result<ProvisionerWorkerStatus, ProvisionerWorkerError> {
    if let Some(error) = worker_error_from_status(response.status()) {
        return Err(error);
    }

    let payload = response
        .json::<ProvisionerWorkerStatusResponse>()
        .await
        .map_err(|_| ProvisionerWorkerError::InvalidPayload)?;
    status_from_response(payload)
}

fn worker_error_from_status(status: StatusCode) -> Option<ProvisionerWorkerError> {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            Some(ProvisionerWorkerError::Unauthorized)
        }
        StatusCode::CONFLICT => Some(ProvisionerWorkerError::Conflict),
        status if !status.is_success() => Some(ProvisionerWorkerError::Unreachable),
        _ => None,
    }
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
    let status = match response.status.as_deref() {
        Some("idle") => ProvisionerWorkerJobStatus::Idle,
        Some("running") => ProvisionerWorkerJobStatus::Running,
        Some("cancelling") => ProvisionerWorkerJobStatus::Cancelling,
        Some("succeeded") => ProvisionerWorkerJobStatus::Succeeded,
        Some("failed") => ProvisionerWorkerJobStatus::Failed,
        _ => return Err(ProvisionerWorkerError::InvalidPayload),
    };
    let phase = match response.phase.as_deref() {
        Some("idle") => ProvisionerWorkerPhase::Idle,
        Some("resolving_workflow") => ProvisionerWorkerPhase::ResolvingWorkflow,
        Some("installing_runtime") => ProvisionerWorkerPhase::InstallingRuntime,
        Some("installing_models") => ProvisionerWorkerPhase::InstallingModels,
        Some("installing_custom_nodes") => ProvisionerWorkerPhase::InstallingCustomNodes,
        Some("writing_manifest") => ProvisionerWorkerPhase::WritingManifest,
        Some("completed") => ProvisionerWorkerPhase::Completed,
        Some("failed") => ProvisionerWorkerPhase::Failed,
        _ => return Err(ProvisionerWorkerError::InvalidPayload),
    };
    if response
        .progress_percent
        .is_some_and(|percent| percent > 100)
    {
        return Err(ProvisionerWorkerError::InvalidPayload);
    }

    let diagnostic = response.diagnostic.map(sanitize_diagnostic);
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

pub fn progress_from_worker_status(
    status: &ProvisionerWorkerStatus,
) -> WorkspaceProvisioningProgress {
    WorkspaceProvisioningProgress {
        status: match status.status {
            ProvisionerWorkerJobStatus::Idle | ProvisionerWorkerJobStatus::Running => {
                WorkspaceProvisioningStatus::Running
            }
            ProvisionerWorkerJobStatus::Cancelling => WorkspaceProvisioningStatus::Cancelling,
            ProvisionerWorkerJobStatus::Succeeded => WorkspaceProvisioningStatus::Running,
            ProvisionerWorkerJobStatus::Failed => WorkspaceProvisioningStatus::Failed,
        },
        phase: match status.phase {
            ProvisionerWorkerPhase::Idle | ProvisionerWorkerPhase::ResolvingWorkflow => {
                WorkspaceProvisioningPhase::PreparingEnvironment
            }
            ProvisionerWorkerPhase::InstallingRuntime
            | ProvisionerWorkerPhase::InstallingModels
            | ProvisionerWorkerPhase::InstallingCustomNodes
            | ProvisionerWorkerPhase::WritingManifest => {
                WorkspaceProvisioningPhase::PreparingEnvironment
            }
            ProvisionerWorkerPhase::Completed => {
                WorkspaceProvisioningPhase::CreatingEndpointTemplate
            }
            ProvisionerWorkerPhase::Failed => WorkspaceProvisioningPhase::Failed,
        },
        percent: status.progress_percent,
        message: status.diagnostic.clone(),
    }
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
    fn rejects_invalid_payloads() {
        let error = status_from_response(ProvisionerWorkerStatusResponse {
            status: Some("running".to_string()),
            phase: Some("installing_models".to_string()),
            progress_percent: Some(101),
            diagnostic: None,
        })
        .expect_err("invalid percentage should fail");

        assert_eq!(error, ProvisionerWorkerError::InvalidPayload);
    }

    #[test]
    fn maps_unauthorized_and_conflict_statuses() {
        assert_eq!(
            worker_error_from_status(StatusCode::UNAUTHORIZED),
            Some(ProvisionerWorkerError::Unauthorized)
        );
        assert_eq!(
            worker_error_from_status(StatusCode::CONFLICT),
            Some(ProvisionerWorkerError::Conflict)
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
}
