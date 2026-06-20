use std::error::Error;

use crate::{
    commands::{
        errors::{CommandError, NativeCommandError},
        types::{
            secrets::SetupApiKeyRequest,
            workspace::{CreateRunpodWorkspaceRequest, WorkspaceIdRequest},
        },
    },
    shared::{error_source_chain, leaf_error_message, new_trace_id},
};

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

pub fn start_command_trace() -> String {
    let trace_id = new_trace_id();
    tracing::Span::current().record("trace_id", trace_id.as_str());
    trace_id
}

pub fn native_command_error<Code>(
    command: &'static str,
    trace_id: &str,
    error: NativeCommandError,
    code: Code,
) -> CommandError<Code>
where
    Code: std::fmt::Debug,
{
    tracing::error!(
        trace_id = %trace_id,
        command = command,
        startup_code = ?error.code,
        code = ?code,
        error = ?error.message,
        "native command failed"
    );

    CommandError::new(code, error.message, trace_id)
}

pub fn command_error<E, Code>(
    command: &'static str,
    trace_id: &str,
    error: E,
    map_code: impl FnOnce(&E) -> Code,
) -> CommandError<Code>
where
    E: Error + 'static,
    Code: std::fmt::Debug,
{
    command_error_with_duration(command, trace_id, error, map_code, None, None)
}

fn command_error_with_duration<E, Code>(
    command: &'static str,
    trace_id: &str,
    error: E,
    map_code: impl FnOnce(&E) -> Code,
    duration_ms: Option<u128>,
    request_metadata: Option<&CommandRequestMetadata>,
) -> CommandError<Code>
where
    E: Error + 'static,
    Code: std::fmt::Debug,
{
    let code = map_code(&error);
    let message = leaf_error_message(&error);
    let source_chain = error_source_chain(&error);

    tracing::error!(
        trace_id = %trace_id,
        command = command,
        duration_ms = duration_ms,
        request_metadata = ?request_metadata,
        code = ?code,
        error = ?message,
        source_chain = ?source_chain,
        "native command failed"
    );

    CommandError::new(code, message, trace_id)
}
