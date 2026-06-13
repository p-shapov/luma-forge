use std::{error::Error, path::PathBuf};

use crate::{
    commands::{
        errors::{NativeCommandError, NativeCommandErrorCode},
        types::{
            secrets::SetupApiKeyRequest,
            workspace::{CreateRunpodWorkspaceRequest, WorkspaceIdRequest},
        },
    },
    domain::{
        lifecycle_operation::{LifecycleOperationPayload, LifecycleOperationState},
        runpod::{
            RunpodCleanupStep, RunpodDeleteStep, RunpodLifecycleOperationPayload,
            RunpodProvisionStep,
        },
    },
};
use tracing_subscriber::{fmt, fmt::format::FmtSpan, EnvFilter};

#[derive(Debug)]
pub struct DiagnosticsGuard {
    _file_guard: Option<tracing_appender::non_blocking::WorkerGuard>,
}

pub fn new_diagnostic_id() -> String {
    format!("diag-{}", uuid::Uuid::new_v4())
}

pub fn init(log_dir: Option<PathBuf>) -> DiagnosticsGuard {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,luma_forge_lib=debug"));

    if let Some(log_dir) = log_dir {
        let file_appender = tracing_appender::rolling::daily(log_dir, "luma-forge.log");
        let (writer, guard) = tracing_appender::non_blocking(file_appender);
        let subscriber = fmt()
            .with_env_filter(filter)
            .with_writer(writer)
            .with_ansi(false)
            .json()
            .with_current_span(true)
            .with_span_list(true)
            .with_span_events(FmtSpan::CLOSE)
            .finish();
        let _ = tracing::subscriber::set_global_default(subscriber);

        return DiagnosticsGuard {
            _file_guard: Some(guard),
        };
    }

    let subscriber = fmt()
        .with_env_filter(filter)
        .with_span_events(FmtSpan::CLOSE)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    DiagnosticsGuard { _file_guard: None }
}

pub fn redact_for_log(input: &str) -> String {
    input
        .split_whitespace()
        .map(|part| {
            let lower = part.to_ascii_lowercase();
            if lower.starts_with("bearer_token=")
                || lower.starts_with("api_key=")
                || lower.starts_with("hugging_face_api_key=")
                || lower.starts_with("authorization=")
            {
                let key = part.split('=').next().unwrap_or(part);
                format!("{key}=[redacted]")
            } else {
                part.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn leaf_error_message(error: &(dyn Error + 'static)) -> String {
    let mut message = error.to_string();
    let mut source = error.source();

    while let Some(error) = source {
        message = error.to_string();
        source = error.source();
    }

    message
}

pub fn error_source_chain(error: &(dyn Error + 'static)) -> Vec<String> {
    let mut codes = Vec::new();
    let mut source = error.source();

    while let Some(error) = source {
        codes.push(format!("{:?}", error_code_for_source(error)));
        source = error.source();
    }

    codes
}

fn error_code_for_source(error: &(dyn Error + 'static)) -> NativeCommandErrorCode {
    if let Some(error) = error.downcast_ref::<crate::runpod_runtime::errors::RunpodRuntimeError>() {
        return NativeCommandErrorCode::from(error);
    }
    if let Some(error) = error.downcast_ref::<crate::secrets_storage::SecretsStorageError>() {
        return NativeCommandErrorCode::from(error);
    }
    if let Some(error) = error.downcast_ref::<crate::shared::ApiError>() {
        return NativeCommandErrorCode::from(error);
    }
    if let Some(error) = error.downcast_ref::<crate::workflow_catalog::WorkflowCatalogError>() {
        return NativeCommandErrorCode::from(error);
    }
    if let Some(error) = error.downcast_ref::<crate::workspace_catalog::WorkspaceCatalogError>() {
        return NativeCommandErrorCode::from(error);
    }

    NativeCommandErrorCode::InvalidRuntimeState
}

pub type CommandRequestMetadata = Vec<(&'static str, String)>;

pub trait CommandRequestLogMetadata {
    fn command_request_metadata(&self) -> CommandRequestMetadata;
}

impl CommandRequestLogMetadata for SetupApiKeyRequest {
    fn command_request_metadata(&self) -> CommandRequestMetadata {
        Vec::new()
    }
}

impl CommandRequestLogMetadata for WorkspaceIdRequest {
    fn command_request_metadata(&self) -> CommandRequestMetadata {
        vec![("workspace_id", self.workspace_id.clone())]
    }
}

impl CommandRequestLogMetadata for CreateRunpodWorkspaceRequest {
    fn command_request_metadata(&self) -> CommandRequestMetadata {
        vec![
            ("workflow_preset_id", self.workflow_preset_id.clone()),
            ("datacenter_id", self.placement.datacenter_id.clone()),
            ("gpu_id", self.placement.gpu_id.clone()),
            ("volume_size_gb", self.placement.volume_size_gb.to_string()),
        ]
    }
}

pub fn command_request_metadata<T>(request: &T) -> CommandRequestMetadata
where
    T: CommandRequestLogMetadata + ?Sized,
{
    request.command_request_metadata()
}

pub fn empty_command_request_metadata() -> CommandRequestMetadata {
    Vec::new()
}

pub fn native_command_error(error: NativeCommandError) -> NativeCommandError {
    let message = redact_for_log(&error.message);
    tracing::error!(
        diagnostic_id = %error.diagnostic_id,
        code = ?error.code,
        error = ?message,
        "native command failed"
    );
    error
}

pub fn command_error<E>(command: &'static str, error: E) -> NativeCommandError
where
    E: Error + 'static,
    for<'a> NativeCommandErrorCode: From<&'a E>,
{
    command_error_with_duration(command, error, None, None)
}

fn command_error_with_duration<E>(
    command: &'static str,
    error: E,
    duration_ms: Option<u128>,
    request_metadata: Option<&CommandRequestMetadata>,
) -> NativeCommandError
where
    E: Error + 'static,
    for<'a> NativeCommandErrorCode: From<&'a E>,
{
    let diagnostic_id = new_diagnostic_id();
    let code = NativeCommandErrorCode::from(&error);
    let message = leaf_error_message(&error);
    let log_message = redact_for_log(&message);
    let source_chain = error_source_chain(&error);

    tracing::error!(
        diagnostic_id = %diagnostic_id,
        command = command,
        duration_ms = duration_ms,
        request_metadata = ?request_metadata,
        code = ?code,
        error = ?log_message,
        source_chain = ?source_chain,
        "native command failed"
    );

    NativeCommandError::new(code, message, diagnostic_id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LifecycleLogFields {
    pub operation_kind: &'static str,
    pub step: Option<&'static str>,
}

pub fn lifecycle_log_fields(payload: &LifecycleOperationPayload) -> LifecycleLogFields {
    match payload {
        LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Provision { step }) => {
            LifecycleLogFields {
                operation_kind: "provision",
                step: step.as_ref().map(provision_step_label),
            }
        }
        LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Cleanup { step }) => {
            LifecycleLogFields {
                operation_kind: "cleanup",
                step: step.as_ref().map(cleanup_step_label),
            }
        }
        LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Delete { step }) => {
            LifecycleLogFields {
                operation_kind: "delete",
                step: step.as_ref().map(delete_step_label),
            }
        }
    }
}

pub fn lifecycle_state_label(state: LifecycleOperationState) -> &'static str {
    match state {
        LifecycleOperationState::Running => "running",
        LifecycleOperationState::Completed => "completed",
        LifecycleOperationState::Failed => "failed",
        LifecycleOperationState::Stale => "stale",
    }
}

fn provision_step_label(step: &RunpodProvisionStep) -> &'static str {
    match step {
        RunpodProvisionStep::CreateNetworkVolume => "create_network_volume",
        RunpodProvisionStep::StartProvisionerPod => "start_provisioner_pod",
        RunpodProvisionStep::PollProvisioner => "poll_provisioner",
        RunpodProvisionStep::TerminateProvisionerPod => "terminate_provisioner_pod",
        RunpodProvisionStep::CreateTemplate => "create_template",
        RunpodProvisionStep::CreateEndpoint => "create_endpoint",
    }
}

fn cleanup_step_label(step: &RunpodCleanupStep) -> &'static str {
    match step {
        RunpodCleanupStep::DeleteEndpoint => "delete_endpoint",
        RunpodCleanupStep::DeleteTemplate => "delete_template",
        RunpodCleanupStep::TerminateProvisionerPod => "terminate_provisioner_pod",
        RunpodCleanupStep::DeleteNetworkVolume => "delete_network_volume",
    }
}

fn delete_step_label(step: &RunpodDeleteStep) -> &'static str {
    match step {
        RunpodDeleteStep::DeleteEndpoint => "delete_endpoint",
        RunpodDeleteStep::DeleteTemplate => "delete_template",
        RunpodDeleteStep::TerminateProvisionerPod => "terminate_provisioner_pod",
        RunpodDeleteStep::DeleteNetworkVolume => "delete_network_volume",
        RunpodDeleteStep::DeleteLocalWorkspace => "delete_local_workspace",
    }
}

pub fn lifecycle_error<E>(
    operation_id: &str,
    workspace_id: Option<&str>,
    payload: Option<&LifecycleOperationPayload>,
    error: &E,
) -> String
where
    E: Error + 'static,
    for<'a> NativeCommandErrorCode: From<&'a E>,
{
    let diagnostic_id = new_diagnostic_id();
    let source_chain = error_source_chain(error);
    let message = leaf_error_message(error);
    let log_message = redact_for_log(&message);
    let code = NativeCommandErrorCode::from(error);
    let fields = payload.map(lifecycle_log_fields);

    tracing::error!(
        diagnostic_id = %diagnostic_id,
        operation_id = operation_id,
        workspace_id = workspace_id.unwrap_or("unknown"),
        operation_kind = fields.map_or("unknown", |fields| fields.operation_kind),
        state = lifecycle_state_label(LifecycleOperationState::Failed),
        step = fields.and_then(|fields| fields.step).unwrap_or("none"),
        error_code = ?code,
        error = ?log_message,
        source_chain = ?source_chain,
        "lifecycle operation failed"
    );

    diagnostic_id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        commands::types::{
            secrets::SetupApiKeyRequest,
            workspace::{CreateRunpodWorkspaceRequest, WorkspaceIdRequest},
        },
        domain::{
            lifecycle_operation::LifecycleOperationPayload,
            runpod::{RunpodLifecycleOperationPayload, RunpodProvisionStep},
        },
    };

    #[test]
    fn redact_known_secret_names() {
        let input = "bearer_token=abc api_key=def hugging_face_api_key=ghi";

        assert_eq!(
            redact_for_log(input),
            "bearer_token=[redacted] api_key=[redacted] hugging_face_api_key=[redacted]"
        );
    }

    #[test]
    fn redact_known_authorization_secret_names() {
        let input = "authorization=Bearer-abc Authorization=Bearer-def";

        assert_eq!(
            redact_for_log(input),
            "authorization=[redacted] Authorization=[redacted]"
        );
    }

    #[test]
    fn source_chain_includes_nested_error_codes() {
        let error = crate::runpod_runtime::errors::RunpodRuntimeError::RunpodApiKeyUnavailable(
            crate::secrets_storage::SecretsStorageError::StoreUnavailable,
        );

        let chain = error_source_chain(&error);

        assert_eq!(chain, vec!["StoreUnavailable".to_string()]);
    }

    #[test]
    fn native_command_error_keeps_existing_diagnostic_id() {
        let error = NativeCommandError::new(
            NativeCommandErrorCode::InvalidRuntimeState,
            "startup failed",
            "diag-existing",
        );

        let converted = native_command_error(error);

        assert_eq!(converted.diagnostic_id, "diag-existing");
        assert_eq!(converted.message, "startup failed");
        assert_eq!(converted.code, NativeCommandErrorCode::InvalidRuntimeState);
    }

    #[test]
    fn leaf_error_message_returns_deepest_source_message() {
        let error = crate::runpod_runtime::errors::RunpodRuntimeError::RunpodApiKeyUnavailable(
            crate::secrets_storage::SecretsStorageError::StoreUnavailable,
        );

        assert_eq!(leaf_error_message(&error), "secure storage is unavailable");
    }

    #[test]
    fn lifecycle_log_fields_describe_payload_without_serializing_it() {
        let payload =
            LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Provision {
                step: Some(RunpodProvisionStep::CreateEndpoint),
            });

        let fields = lifecycle_log_fields(&payload);

        assert_eq!(fields.operation_kind, "provision");
        assert_eq!(fields.step, Some("create_endpoint"));
    }

    #[test]
    fn command_request_metadata_omits_secret_values() {
        let request = SetupApiKeyRequest {
            api_key: "secret-token".to_string(),
        };

        let metadata = command_request_metadata(&request);

        assert!(metadata.is_empty());
        assert!(!format!("{metadata:?}").contains("secret-token"));
    }

    #[test]
    fn empty_command_request_metadata_is_stable() {
        let metadata = empty_command_request_metadata();

        assert!(metadata.is_empty());
    }

    #[test]
    fn command_request_metadata_includes_workspace_id() {
        let request = WorkspaceIdRequest {
            workspace_id: "workspace-1".to_string(),
        };

        let metadata = command_request_metadata(&request);

        assert_eq!(metadata, vec![("workspace_id", "workspace-1".to_string())]);
    }

    #[test]
    fn command_request_metadata_includes_safe_create_workspace_fields() {
        let request = CreateRunpodWorkspaceRequest {
            workflow_preset_id: "preset-1".to_string(),
            placement: crate::commands::types::placement::RunpodPlacementPlanInput {
                datacenter_id: "dc-1".to_string(),
                gpu_id: "gpu-1".to_string(),
                volume_size_gb: 100,
                keep_alive_limits: None,
            },
        };

        let metadata = command_request_metadata(&request);

        assert_eq!(
            metadata,
            vec![
                ("workflow_preset_id", "preset-1".to_string()),
                ("datacenter_id", "dc-1".to_string()),
                ("gpu_id", "gpu-1".to_string()),
                ("volume_size_gb", "100".to_string())
            ]
        );
    }
}
