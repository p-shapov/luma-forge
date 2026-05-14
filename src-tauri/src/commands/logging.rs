use std::time::Instant;

use crate::commands::{error::NativeCommandError, CommandResult};

#[derive(Debug)]
pub(crate) struct CommandLog {
    command_name: &'static str,
    operation_id: uuid::Uuid,
    started_at: Instant,
    fields: Vec<CommandLogField>,
}

#[derive(Debug)]
struct CommandLogField {
    key: &'static str,
    value: String,
}

impl CommandLog {
    pub(crate) fn new(command_name: &'static str) -> Self {
        Self {
            command_name,
            operation_id: uuid::Uuid::new_v4(),
            started_at: Instant::now(),
            fields: Vec::new(),
        }
    }

    pub(crate) fn start(self) -> Self {
        log::info!("{}", self.start_message());
        self
    }

    pub(crate) fn with_provider_id(mut self, provider_id: &'static str) -> Self {
        self.fields.push(CommandLogField {
            key: "provider_id",
            value: safe_log_value(provider_id),
        });
        self
    }

    pub(crate) fn finish<T>(&self, result: CommandResult<T>) -> CommandResult<T> {
        match &result {
            Ok(_) => log::info!("{}", self.success_message()),
            Err(error) => log::warn!("{}", self.failure_message(error)),
        }

        result
    }

    fn start_message(&self) -> String {
        format!(
            "native_command_start command={} operation_id={}{}",
            self.command_name,
            self.operation_id,
            self.fields_message()
        )
    }

    fn success_message(&self) -> String {
        format!(
            "native_command_finish command={} operation_id={} outcome=ok elapsed_ms={}{}",
            self.command_name,
            self.operation_id,
            self.elapsed_ms(),
            self.fields_message()
        )
    }

    fn failure_message(&self, error: &NativeCommandError) -> String {
        format!(
            "native_command_finish command={} operation_id={} outcome=error elapsed_ms={}{} code={} retryable={}{}{}{}",
            self.command_name,
            self.operation_id,
            self.elapsed_ms(),
            self.fields_message(),
            error.code.as_str(),
            error.retryable,
            optional_field("field", error.field.as_deref()),
            optional_field("reason", error.reason.as_deref()),
            optional_field("recovery_action", error.recovery_action.as_deref())
        )
    }

    fn elapsed_ms(&self) -> u128 {
        self.started_at.elapsed().as_millis()
    }

    fn fields_message(&self) -> String {
        self.fields
            .iter()
            .map(|field| format!(" {}={}", field.key, field.value))
            .collect()
    }

    #[cfg(test)]
    fn for_test(command_name: &'static str, operation_id: uuid::Uuid) -> Self {
        Self {
            command_name,
            operation_id,
            started_at: Instant::now(),
            fields: Vec::new(),
        }
    }
}

fn optional_field(key: &str, value: Option<&str>) -> String {
    value
        .map(|value| format!(" {key}={}", safe_log_value(value)))
        .unwrap_or_default()
}

fn safe_log_value(value: impl AsRef<str>) -> String {
    value
        .as_ref()
        .chars()
        .take(128)
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':' | '@')
            {
                character
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::error::NativeCommandErrorCode;

    #[test]
    fn failure_message_includes_only_safe_error_metadata() {
        let log = CommandLog::for_test("setup_gpu_cloud_provider", uuid::Uuid::nil())
            .with_provider_id("runpod");
        let error = NativeCommandError {
            code: NativeCommandErrorCode::ProviderApiKeyUnauthorized,
            message: "Provider API key is not authorized.".to_string(),
            retryable: false,
            field: Some("provider_api_key".to_string()),
            reason: Some("provider_rejected_key".to_string()),
            recovery_action: Some("enter_provider_api_key".to_string()),
        };

        let message = log.failure_message(&error);

        assert!(message.contains("command=setup_gpu_cloud_provider"));
        assert!(message.contains("operation_id=00000000-0000-0000-0000-000000000000"));
        assert!(message.contains("provider_id=runpod"));
        assert!(message.contains("code=provider_api_key_unauthorized"));
        assert!(message.contains("field=provider_api_key"));
        assert!(message.contains("reason=provider_rejected_key"));
        assert!(message.contains("recovery_action=enter_provider_api_key"));
        assert!(!message.contains("Provider API key is not authorized."));
        assert!(!message.contains("rp_test_secret_key"));
    }

    #[test]
    fn optional_error_fields_are_sanitized_for_single_line_logs() {
        let value = safe_log_value("provider\napi key");

        assert_eq!(value, "provider_api_key");
    }
}
