use std::future::Future;

use fastrace::{collector::SpanContext, future::FutureExt, Span};

use crate::{
    diagnostics,
    tauri_api::{errors::CommandErrorCode, CommandResult},
};

const TRACE_UNAVAILABLE: &str = "trace-unavailable";

pub(crate) async fn run_async_command<T, Code, Fut>(
    name: &'static str,
    handler: impl FnOnce(String) -> Fut,
) -> CommandResult<T, Code>
where
    Code: CommandErrorCode,
    Fut: Future<Output = CommandResult<T, Code>>,
{
    let root = Span::root(name, SpanContext::random());
    let trace_id =
        diagnostics::trace_id_from_span(&root).unwrap_or_else(|| TRACE_UNAVAILABLE.to_string());

    async move {
        log::info!(command = name; "tauri command started");
        let result = handler(trace_id.clone()).await;
        log_command_result(name, &result);
        result
    }
    .in_span(root)
    .await
}

pub(crate) fn run_sync_command<T, Code>(
    name: &'static str,
    handler: impl FnOnce(String) -> CommandResult<T, Code>,
) -> CommandResult<T, Code>
where
    Code: CommandErrorCode,
{
    let root = Span::root(name, SpanContext::random());
    let trace_id =
        diagnostics::trace_id_from_span(&root).unwrap_or_else(|| TRACE_UNAVAILABLE.to_string());
    let _guard = root.set_local_parent();

    log::info!(command = name; "tauri command started");
    let result = handler(trace_id);
    log_command_result(name, &result);
    result
}

fn log_command_result<T, Code>(name: &'static str, result: &CommandResult<T, Code>)
where
    Code: CommandErrorCode,
{
    match result {
        Ok(_) => log::info!(command = name; "tauri command completed"),
        Err(error) => {
            log::error!(
                command = name,
                error = diagnostics::error_diagnostics_log_json(&error.diagnostics);
                "tauri command failed"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, Once};

    use crate::diagnostics::{ErrorDiagnosticFrame, ErrorDiagnostics};

    static TEST_LOGGER: CapturingLogger = CapturingLogger {
        records: Mutex::new(Vec::new()),
    };
    static INIT_LOGGER: Once = Once::new();

    struct CapturingLogger {
        records: Mutex<Vec<CapturedRecord>>,
    }

    #[derive(Debug)]
    struct CapturedRecord {
        message: String,
        key_values: Vec<(String, String)>,
    }

    impl log::Log for CapturingLogger {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            metadata.level() <= log::Level::Info
        }

        fn log(&self, record: &log::Record<'_>) {
            if !self.enabled(record.metadata()) {
                return;
            }

            let mut key_values = CapturedKeyValues::default();
            let _ = record.key_values().visit(&mut key_values);
            self.records.lock().unwrap().push(CapturedRecord {
                message: record.args().to_string(),
                key_values: key_values.0,
            });
        }

        fn flush(&self) {}
    }

    #[derive(Default)]
    struct CapturedKeyValues(Vec<(String, String)>);

    impl<'kvs> log::kv::VisitSource<'kvs> for CapturedKeyValues {
        fn visit_pair(
            &mut self,
            key: log::kv::Key<'kvs>,
            value: log::kv::Value<'kvs>,
        ) -> Result<(), log::kv::Error> {
            self.0.push((key.as_str().to_string(), value.to_string()));
            Ok(())
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum TestCommandErrorCode {
        CommandError,
    }

    impl CommandErrorCode for TestCommandErrorCode {
        fn from_diagnostics_code(_code: &str) -> Self {
            Self::CommandError
        }
    }

    fn init_test_logger() {
        INIT_LOGGER.call_once(|| {
            let _ = log::set_logger(&TEST_LOGGER);
            log::set_max_level(log::LevelFilter::Info);
        });
        TEST_LOGGER.records.lock().unwrap().clear();
    }

    #[test]
    fn failed_command_logs_error_without_trace_id_key_value() {
        init_test_logger();
        let result: CommandResult<(), TestCommandErrorCode> =
            Err(crate::tauri_api::CommandError::new(
                TestCommandErrorCode::CommandError,
                "runpod placement options failed",
                "trace-123",
                ErrorDiagnostics {
                    code: "runtime_provider_api_key_unavailable".to_string(),
                    message: "runpod placement options failed".to_string(),
                    chain: vec![ErrorDiagnosticFrame {
                        code: "runtime_provider_api_key_unavailable".to_string(),
                        message: "runtime provider api key unavailable".to_string(),
                    }],
                },
            ));

        log_command_result("get_runpod_placement_options", &result);

        let records = TEST_LOGGER.records.lock().unwrap();
        let record = records
            .iter()
            .find(|record| record.message == "tauri command failed")
            .expect("failed command should be logged");

        assert!(record.key_values.iter().any(|(key, _)| key == "command"));
        assert!(record.key_values.iter().any(|(key, _)| key == "error"));
        assert!(!record
            .key_values
            .iter()
            .any(|(key, _)| key == "trace_id" || key == "trace-id"));
    }
}
