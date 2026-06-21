use std::{error::Error, path::Path};

use fastrace::{collector::SpanContext, Span};
use logforth::filter::FilterResult;
use logforth::record::{FilterCriteria, Level, LevelFilter};

const REDACTED: &str = "[REDACTED]";

#[derive(Debug, thiserror::Error)]
pub enum DiagnosticsInitializationError {
    #[error("native diagnostics could not be initialized: {message}")]
    SetupFailed { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorDiagnostics {
    pub code: String,
    pub message: String,
    pub cause: String,
    pub source_chain: Vec<String>,
}

pub fn init(logs_dir: &Path) -> Result<(), DiagnosticsInitializationError> {
    let appender = logforth::append::file::FileBuilder::new(logs_dir, "luma-forge.log")
        .layout(logforth::layout::JsonLayout::default())
        .build()
        .map_err(|error| DiagnosticsInitializationError::SetupFailed {
            message: error.to_string(),
        })?;

    let logger = logforth::bridge::log::LogBridge::new(
        logforth::core::builder()
            .dispatch(|dispatch| {
                dispatch
                    .filter(AppLogFilter)
                    .diagnostic(logforth::diagnostic::FastraceDiagnostic::default())
                    .append(appender)
            })
            .build(),
    );

    log::set_boxed_logger(Box::new(logger)).map_err(|error| {
        DiagnosticsInitializationError::SetupFailed {
            message: error.to_string(),
        }
    })?;
    log::set_max_level(log::LevelFilter::Info);

    Ok(())
}

pub fn trace_id_from_context(context: &SpanContext) -> String {
    context.trace_id.to_string()
}

pub fn trace_id_from_span(span: &Span) -> Option<String> {
    SpanContext::from_span(span).map(|context| trace_id_from_context(&context))
}

pub fn current_trace_id() -> Option<String> {
    SpanContext::current_local_parent().map(|context| trace_id_from_context(&context))
}

pub fn error_diagnostics<E>(error: &E, fallback_code: &'static str) -> ErrorDiagnostics
where
    E: Error + serde::Serialize + 'static,
{
    let mut source_chain = Vec::new();
    let mut leaf: &(dyn Error + 'static) = error;
    while let Some(source) = leaf.source() {
        source_chain.push(sanitize_diagnostic_string(&source.to_string()));
        leaf = source;
    }

    ErrorDiagnostics {
        code: serialized_error_code(error, fallback_code),
        message: sanitize_diagnostic_string(&error.to_string()),
        cause: sanitize_diagnostic_string(&leaf.to_string()),
        source_chain,
    }
}

fn serialized_error_code(error: &impl serde::Serialize, fallback: &'static str) -> String {
    match serde_json::to_value(error) {
        Ok(serde_json::Value::String(code)) => code,
        Ok(serde_json::Value::Object(fields)) => fields
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| fallback.to_string()),
        _ => fallback.to_string(),
    }
}

#[derive(Debug)]
struct AppLogFilter;

impl logforth::Filter for AppLogFilter {
    fn enabled(
        &self,
        criteria: &FilterCriteria,
        _: &[Box<dyn logforth::Diagnostic>],
    ) -> FilterResult {
        if !LevelFilter::MoreSevereEqual(Level::Info).test(criteria.level()) {
            return FilterResult::Reject;
        }

        if is_app_log_target(criteria.target()) {
            FilterResult::Neutral
        } else {
            FilterResult::Reject
        }
    }
}

fn is_app_log_target(target: &str) -> bool {
    matches!(target, "luma_forge" | "luma_forge_lib")
        || target.starts_with("luma_forge::")
        || target.starts_with("luma_forge_lib::")
}

fn sanitize_diagnostic_string(value: &str) -> String {
    let mut sanitized = redact_header_value(value, "authorization:");
    sanitized = redact_bearer_token(&sanitized);

    for key in [
        "access_token",
        "api_key",
        "token",
        "x-amz-signature",
        "x-amz-credential",
        "x-goog-signature",
    ] {
        sanitized = redact_key_value(&sanitized, key);
    }

    redact_hugging_face_token(&sanitized)
}

fn redact_bearer_token(value: &str) -> String {
    redact_prefixed_secret(value, "bearer ", true)
}

fn redact_hugging_face_token(value: &str) -> String {
    redact_prefixed_secret(value, "hf_", false)
}

fn redact_prefixed_secret(value: &str, prefix: &str, keep_prefix: bool) -> String {
    let mut sanitized = value.to_string();
    let mut search_from = 0;

    while let Some(relative_start) = find_case_insensitive(&sanitized[search_from..], prefix) {
        let start = search_from + relative_start;
        let secret_start = start + prefix.len();
        let secret_end = find_secret_end(&sanitized, secret_start);

        if secret_start == secret_end {
            search_from = secret_start;
            continue;
        }

        let replacement = if keep_prefix {
            format!("{}{}", &sanitized[start..secret_start], REDACTED)
        } else {
            REDACTED.to_string()
        };

        sanitized.replace_range(start..secret_end, &replacement);
        search_from = start + replacement.len();
    }

    sanitized
}

fn redact_header_value(value: &str, header_name: &str) -> String {
    let mut sanitized = value.to_string();
    let mut search_from = 0;

    while let Some(relative_start) = find_case_insensitive(&sanitized[search_from..], header_name) {
        let start = search_from + relative_start;
        let header_end = start + header_name.len();
        let value_start = skip_spaces(&sanitized, header_end);
        let value_end = sanitized[value_start..]
            .find(['\n', '\r'])
            .map(|offset| value_start + offset)
            .unwrap_or_else(|| sanitized.len());

        sanitized.replace_range(value_start..value_end, REDACTED);
        search_from = value_start + REDACTED.len();
    }

    sanitized
}

fn redact_key_value(value: &str, key: &str) -> String {
    let pattern = format!("{key}=");
    let mut sanitized = value.to_string();
    let mut search_from = 0;

    while let Some(relative_start) = find_case_insensitive(&sanitized[search_from..], &pattern) {
        let start = search_from + relative_start;

        if has_identifier_prefix(&sanitized, start) {
            search_from = start + pattern.len();
            continue;
        }

        let secret_start = start + pattern.len();
        let secret_end = sanitized[secret_start..]
            .find(['&', ' ', '\t', '\n', '\r', '"', '\'', ')', ']', '}'])
            .map(|offset| secret_start + offset)
            .unwrap_or_else(|| sanitized.len());

        sanitized.replace_range(secret_start..secret_end, REDACTED);
        search_from = secret_start + REDACTED.len();
    }

    sanitized
}

fn has_identifier_prefix(value: &str, index: usize) -> bool {
    value[..index]
        .chars()
        .next_back()
        .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn skip_spaces(value: &str, index: usize) -> usize {
    let mut current = index;
    while let Some(character) = value[current..].chars().next() {
        if !character.is_ascii_whitespace() || matches!(character, '\n' | '\r') {
            break;
        }
        current += character.len_utf8();
    }
    current
}

fn find_secret_end(value: &str, index: usize) -> usize {
    value[index..]
        .find([
            ' ', '\t', '\n', '\r', ',', ';', '"', '\'', ')', ']', '}', '<', '>',
        ])
        .map(|offset| index + offset)
        .unwrap_or_else(|| value.len())
}

fn find_case_insensitive(value: &str, pattern: &str) -> Option<usize> {
    value
        .to_ascii_lowercase()
        .find(&pattern.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;

    use logforth::filter::FilterResult;
    use logforth::record::{FilterCriteria, Level};
    use logforth::Filter;
    use serde::Serialize;

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "snake_case")]
    enum TestError {
        UnitFailure,
        StructuredFailure {
            message: String,
            #[serde(skip)]
            source: Option<Box<TestError>>,
        },
    }

    impl fmt::Display for TestError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                TestError::UnitFailure => formatter.write_str("unit failure"),
                TestError::StructuredFailure { message, .. } => formatter.write_str(message),
            }
        }
    }

    impl Error for TestError {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            match self {
                TestError::UnitFailure => None,
                TestError::StructuredFailure { source, .. } => source
                    .as_ref()
                    .map(|source| source.as_ref() as &(dyn Error + 'static)),
            }
        }
    }

    #[test]
    fn error_diagnostics_reports_code_message_leaf_cause_and_ordered_sources() {
        let error = TestError::StructuredFailure {
            message: "top-level".to_string(),
            source: Some(Box::new(TestError::StructuredFailure {
                message: "middle".to_string(),
                source: Some(Box::new(TestError::StructuredFailure {
                    message: "leaf".to_string(),
                    source: None,
                })),
            })),
        };

        let diagnostics = error_diagnostics(&error, "fallback");

        assert_eq!(diagnostics.code, "structured_failure");
        assert_eq!(diagnostics.message, "top-level");
        assert_eq!(diagnostics.cause, "leaf");
        assert_eq!(diagnostics.source_chain, vec!["middle", "leaf"]);
    }

    #[test]
    fn error_diagnostics_reports_unit_variant_code() {
        let diagnostics = error_diagnostics(&TestError::UnitFailure, "fallback");

        assert_eq!(diagnostics.code, "unit_failure");
        assert_eq!(diagnostics.message, "unit failure");
        assert_eq!(diagnostics.cause, "unit failure");
        assert!(diagnostics.source_chain.is_empty());
    }

    #[test]
    fn error_diagnostics_redacts_sensitive_strings_in_message_cause_and_sources() {
        let error = TestError::StructuredFailure {
            message: "request failed for Bearer top-secret-token at https://example.com/download?api_key=runpod-secret&foo=bar".to_string(),
            source: Some(Box::new(TestError::StructuredFailure {
                message: "Authorization: Basic super-secret".to_string(),
                source: Some(Box::new(TestError::StructuredFailure {
                    message: "worker rejected hf_abcdefghijklmnopqrstuvwxyz123456 access_token=native-secret".to_string(),
                    source: None,
                })),
            })),
        };

        let diagnostics = error_diagnostics(&error, "fallback");

        assert_eq!(diagnostics.code, "structured_failure");
        assert_eq!(
            diagnostics.message,
            "request failed for Bearer [REDACTED] at https://example.com/download?api_key=[REDACTED]&foo=bar"
        );
        assert_eq!(
            diagnostics.cause,
            "worker rejected [REDACTED] access_token=[REDACTED]"
        );
        assert_eq!(
            diagnostics.source_chain,
            vec![
                "Authorization: [REDACTED]",
                "worker rejected [REDACTED] access_token=[REDACTED]",
            ]
        );
        assert!(!diagnostics.message.contains("top-secret-token"));
        assert!(!diagnostics.message.contains("runpod-secret"));
        assert!(!diagnostics
            .cause
            .contains("abcdefghijklmnopqrstuvwxyz123456"));
        assert!(!diagnostics.cause.contains("native-secret"));
    }

    #[test]
    fn app_log_filter_rejects_non_app_targets_and_sub_info_levels() {
        let filter = AppLogFilter;

        let app_info = FilterCriteria::builder()
            .level(Level::Info)
            .target("luma_forge_lib::provider")
            .build();
        let app_error = FilterCriteria::builder()
            .level(Level::Error)
            .target("luma_forge::tauri")
            .build();
        let app_debug = FilterCriteria::builder()
            .level(Level::Debug)
            .target("luma_forge_lib::provider")
            .build();
        let dependency_info = FilterCriteria::builder()
            .level(Level::Info)
            .target("reqwest::connect")
            .build();

        assert_eq!(filter.enabled(&app_info, &[]), FilterResult::Neutral);
        assert_eq!(filter.enabled(&app_error, &[]), FilterResult::Neutral);
        assert_eq!(filter.enabled(&app_debug, &[]), FilterResult::Reject);
        assert_eq!(filter.enabled(&dependency_info, &[]), FilterResult::Reject);
    }
}
