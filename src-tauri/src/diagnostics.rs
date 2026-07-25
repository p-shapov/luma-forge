use std::path::Path;

use logforth::filter::FilterResult;
use logforth::record::{FilterCriteria, Level, LevelFilter};

#[derive(Debug, thiserror::Error)]
pub enum DiagnosticsInitializationError {
    #[error("native diagnostics could not be initialized: {message}")]
    SetupFailed { message: String },
    #[error("native diagnostics logger could not be installed: {message}")]
    InstallFailed { message: String },
}

#[derive(Debug)]
struct ApplicationFilter;

impl logforth::Filter for ApplicationFilter {
    fn enabled(
        &self,
        criteria: &FilterCriteria<'_>,
        _: &[Box<dyn logforth::Diagnostic>],
    ) -> FilterResult {
        if !LevelFilter::MoreSevereEqual(Level::Info).test(criteria.level()) {
            return FilterResult::Reject;
        }

        match criteria.target() {
            "luma_forge" | "luma_forge_lib" => FilterResult::Neutral,
            target
                if target.starts_with("luma_forge::") || target.starts_with("luma_forge_lib::") =>
            {
                FilterResult::Neutral
            }
            _ => FilterResult::Reject,
        }
    }
}

pub fn init(log_path: &Path) -> Result<(), DiagnosticsInitializationError> {
    let directory =
        log_path
            .parent()
            .ok_or_else(|| DiagnosticsInitializationError::SetupFailed {
                message: "diagnostics path has no parent".to_owned(),
            })?;
    let file_name = log_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| DiagnosticsInitializationError::SetupFailed {
            message: "diagnostics file name is invalid".to_owned(),
        })?;
    let appender = logforth::append::file::FileBuilder::new(directory, file_name)
        .layout(logforth::layout::JsonLayout::default())
        .build()
        .map_err(|error| DiagnosticsInitializationError::SetupFailed {
            message: error.to_string(),
        })?;
    let logger = logforth::bridge::log::LogBridge::new(
        logforth::core::builder()
            .dispatch(|dispatch| {
                dispatch
                    .filter(ApplicationFilter)
                    .diagnostic(logforth::diagnostic::FastraceDiagnostic::default())
                    .append(appender)
            })
            .build(),
    );

    log::set_boxed_logger(Box::new(logger)).map_err(|error| {
        DiagnosticsInitializationError::InstallFailed {
            message: error.to_string(),
        }
    })?;
    log::set_max_level(log::LevelFilter::Info);
    Ok(())
}

#[cfg(test)]
mod tests {
    use fastrace::collector::SpanContext;

    #[test]
    fn standard_json_layout_preserves_call_fields() {
        use logforth::kv::{Key, Value};
        use logforth::Layout;

        let fields = [
            (Key::new("function"), Value::static_str("successful")),
            (Key::new("workspace_id"), Value::static_str("workspace-1")),
        ];
        let record = logforth::record::Record::builder()
            .payload(format_args!("call.start"))
            .key_values(fields.as_slice())
            .build();
        let formatted = logforth::layout::JsonLayout::default()
            .format(&record, &[])
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&formatted).unwrap();

        assert_eq!(json["message"], "call.start");
        assert_eq!(json["kvs"]["function"], "successful");
        assert_eq!(json["kvs"]["workspace_id"], "workspace-1");
    }

    #[test]
    fn initialization_reports_file_setup_failure_without_installing_logger() {
        let temp_dir = std::env::temp_dir().join(format!("luma-forge-{}", uuid::Uuid::new_v4()));
        std::fs::write(&temp_dir, b"not a directory").unwrap();

        let result = super::init(&temp_dir.join("diagnostics.log"));

        std::fs::remove_file(temp_dir).unwrap();
        assert!(matches!(
            result,
            Err(super::DiagnosticsInitializationError::SetupFailed { .. })
        ));
    }

    const INSTALL_FAILURE_SUBPROCESS: &str = "LUMA_FORGE_INSTALL_FAILURE_SUBPROCESS";
    const INSTALL_FAILURE_LOGS_DIR: &str = "LUMA_FORGE_INSTALL_FAILURE_LOGS_DIR";
    const TRACE_FIELDS_SUBPROCESS: &str = "LUMA_FORGE_TRACE_FIELDS_SUBPROCESS";
    const TRACE_FIELDS_LOGS_DIR: &str = "LUMA_FORGE_TRACE_FIELDS_LOGS_DIR";

    #[test]
    fn initialization_reports_logger_install_failure_in_subprocess() {
        let logs_dir = std::env::temp_dir().join(format!("luma-forge-{}", uuid::Uuid::new_v4()));
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "diagnostics::tests::initialization_install_failure_subprocess_helper",
                "--nocapture",
            ])
            .env(INSTALL_FAILURE_SUBPROCESS, "1")
            .env(INSTALL_FAILURE_LOGS_DIR, &logs_dir)
            .status()
            .unwrap();

        std::fs::remove_dir_all(logs_dir).unwrap();
        assert!(status.success());
    }

    #[test]
    fn initialization_install_failure_subprocess_helper() {
        if std::env::var_os(INSTALL_FAILURE_SUBPROCESS).is_none() {
            return;
        }

        let temp_dir = std::env::var_os(INSTALL_FAILURE_LOGS_DIR).unwrap();
        let log_path = std::path::Path::new(&temp_dir).join("diagnostics.log");
        super::init(&log_path).unwrap();
        let result = super::init(&log_path);

        assert!(matches!(
            result,
            Err(super::DiagnosticsInitializationError::InstallFailed { .. })
        ));
    }

    #[test]
    fn initialized_logger_writes_trace_and_span_ids_in_subprocess() {
        let logs_dir = std::env::temp_dir().join(format!("luma-forge-{}", uuid::Uuid::new_v4()));
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "diagnostics::tests::initialized_logger_trace_fields_subprocess_helper",
                "--nocapture",
            ])
            .env(TRACE_FIELDS_SUBPROCESS, "1")
            .env(TRACE_FIELDS_LOGS_DIR, &logs_dir)
            .status()
            .unwrap();

        std::fs::remove_dir_all(logs_dir).unwrap();
        assert!(status.success());
    }

    #[test]
    fn initialized_logger_trace_fields_subprocess_helper() {
        if std::env::var_os(TRACE_FIELDS_SUBPROCESS).is_none() {
            return;
        }

        let temp_dir = std::env::var_os(TRACE_FIELDS_LOGS_DIR).unwrap();
        let log_path = std::path::Path::new(&temp_dir).join("diagnostics.log");
        super::init(&log_path).unwrap();
        assert!(log_path.is_file());
        let span = fastrace::Span::root("diagnostics.file_test", SpanContext::random());
        let _guard = span.set_local_parent();
        log::info!("trace fields");
        log::logger().flush();

        let record = std::fs::read_to_string(log_path).unwrap();
        let json: serde_json::Value = serde_json::from_str(record.trim()).unwrap();

        assert!(json["diags"]["trace_id"].is_string());
        assert!(json["diags"]["span_id"].is_string());
    }

    #[test]
    fn application_filter_accepts_info_and_rejects_other_targets_or_debug() {
        use logforth::filter::FilterResult;
        use logforth::record::{FilterCriteria, Level};
        use logforth::Filter;

        let criteria = |target, level| {
            FilterCriteria::builder()
                .target(target)
                .level(level)
                .build()
        };

        assert_eq!(
            super::ApplicationFilter
                .enabled(&criteria("luma_forge_lib::diagnostics", Level::Info), &[]),
            FilterResult::Neutral
        );
        assert_eq!(
            super::ApplicationFilter.enabled(&criteria("dependency", Level::Info), &[]),
            FilterResult::Reject
        );
        assert_eq!(
            super::ApplicationFilter
                .enabled(&criteria("luma_forge_lib::diagnostics", Level::Debug), &[],),
            FilterResult::Reject
        );
    }
}
