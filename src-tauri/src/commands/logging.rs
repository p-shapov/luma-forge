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
