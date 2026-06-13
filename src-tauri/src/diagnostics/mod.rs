use std::{error::Error, path::PathBuf};

use crate::commands::errors::{NativeCommandError, NativeCommandErrorCode};
use tracing_subscriber::{fmt, EnvFilter};

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
            .finish();
        let _ = tracing::subscriber::set_global_default(subscriber);

        return DiagnosticsGuard {
            _file_guard: Some(guard),
        };
    }

    let subscriber = fmt().with_env_filter(filter).finish();
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

pub fn error_source_chain(error: &(dyn Error + 'static)) -> Vec<String> {
    let mut messages = Vec::new();
    let mut source = error.source();

    while let Some(error) = source {
        messages.push(redact_for_log(&error.to_string()));
        source = error.source();
    }

    messages
}

pub fn command_error<E>(command: &'static str, error: E) -> NativeCommandError
where
    E: Error + 'static,
    for<'a> NativeCommandErrorCode: From<&'a E>,
{
    let diagnostic_id = new_diagnostic_id();
    let code = NativeCommandErrorCode::from(&error);
    let message = error.to_string();
    let source_chain = error_source_chain(&error);

    tracing::error!(
        diagnostic_id = %diagnostic_id,
        command = command,
        code = ?code,
        error = %redact_for_log(&message),
        source_chain = ?source_chain,
        "native command failed"
    );

    NativeCommandError::new(code, message, diagnostic_id)
}

pub fn lifecycle_error(
    operation_id: &str,
    workspace_id: Option<&str>,
    error: &(dyn Error + 'static),
) -> String {
    let diagnostic_id = new_diagnostic_id();
    let source_chain = error_source_chain(error);
    let message = error.to_string();

    tracing::error!(
        diagnostic_id = %diagnostic_id,
        operation_id = operation_id,
        workspace_id = workspace_id.unwrap_or("unknown"),
        error = %redact_for_log(&message),
        source_chain = ?source_chain,
        "lifecycle operation failed"
    );

    diagnostic_id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_known_secret_names() {
        let input = "bearer_token=abc api_key=def hugging_face_api_key=ghi";

        assert_eq!(
            redact_for_log(input),
            "bearer_token=[redacted] api_key=[redacted] hugging_face_api_key=[redacted]"
        );
    }

    #[test]
    fn source_chain_includes_nested_error_messages() {
        let error = crate::runpod_runtime::errors::RunpodRuntimeError::RunpodApiKeyUnavailable(
            crate::secrets_storage::SecretsStorageError::StoreUnavailable,
        );

        let chain = error_source_chain(&error);

        assert!(chain
            .iter()
            .any(|message| message.contains("secure storage is unavailable")));
    }
}
