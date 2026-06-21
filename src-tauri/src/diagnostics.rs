use std::{error::Error, path::Path};

use fastrace::{collector::SpanContext, Span};
use logforth::filter::FilterResult;
use logforth::kv::{KeyView, ValueView, Visitor};
use logforth::record::{FilterCriteria, Level, LevelFilter};
use serde::Serialize;
use serde_json::Map;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

const REDACTED: &str = "[REDACTED]";
const REDACTED_BODY: &str = "[REDACTED_BODY]";
const REDACTED_LARGE_DIAGNOSTIC: &str = "[REDACTED_LARGE_DIAGNOSTIC]";
const REDACTED_URL: &str = "[REDACTED_URL]";
const MAX_DIAGNOSTIC_STRING_LEN: usize = 2048;

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
        .layout(SanitizingJsonLayout)
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
    if value.len() > MAX_DIAGNOSTIC_STRING_LEN {
        return REDACTED_LARGE_DIAGNOSTIC.to_string();
    }

    if looks_like_raw_body(value) {
        return REDACTED_BODY.to_string();
    }

    let mut sanitized = redact_signed_url_tokens(value);
    sanitized = redact_header_value(&sanitized, "authorization:");
    sanitized = redact_bearer_token(&sanitized);

    for key in plain_text_sensitive_keys() {
        sanitized = redact_key_value(&sanitized, key);
    }

    redact_hugging_face_token(&sanitized)
}

#[derive(Debug, Clone, Default)]
struct SanitizingJsonLayout;

#[derive(Debug, Default)]
struct JsonValueCollector {
    kvs: Map<String, serde_json::Value>,
}

impl Visitor for JsonValueCollector {
    fn visit(&mut self, key: KeyView, value: ValueView) -> Result<(), logforth::Error> {
        let key = key.to_string();
        let value = match serde_json::to_value(&value) {
            Ok(value) => sanitize_json_value_for_key(Some(key.as_str()), value),
            Err(_) => sanitize_json_value_for_key(
                Some(key.as_str()),
                serde_json::Value::String(value.to_string()),
            ),
        };
        self.kvs.insert(key, value);
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
struct SanitizedRecordLine {
    timestamp: String,
    level: &'static str,
    target: String,
    file: String,
    line: u32,
    message: String,
    #[serde(skip_serializing_if = "Map::is_empty")]
    kvs: Map<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Map::is_empty")]
    diags: Map<String, serde_json::Value>,
}

impl logforth::Layout for SanitizingJsonLayout {
    fn format(
        &self,
        record: &logforth::record::Record,
        diags: &[Box<dyn logforth::Diagnostic>],
    ) -> Result<Vec<u8>, logforth::Error> {
        let timestamp: OffsetDateTime = record.time().into();
        let timestamp = timestamp
            .format(&Rfc3339)
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());

        let mut kvs_collector = JsonValueCollector::default();
        record.key_values().visit(&mut kvs_collector)?;

        let mut diags_collector = JsonValueCollector::default();
        for diagnostic in diags {
            diagnostic.visit(&mut diags_collector)?;
        }

        let record_line = SanitizedRecordLine {
            timestamp,
            level: record.level().name(),
            target: record.target().to_string(),
            file: record.file().unwrap_or_default().to_string(),
            line: record.line().unwrap_or_default(),
            message: sanitize_diagnostic_string(&record.payload().to_string()),
            kvs: kvs_collector.kvs,
            diags: diags_collector.kvs,
        };

        serde_json::to_vec(&record_line).map_err(|error| {
            logforth::Error::new("failed to serialize sanitized diagnostics log record")
                .with_source(error)
        })
    }
}

fn looks_like_raw_body(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }

    let lower = value.to_ascii_lowercase();
    for marker in [
        "request body:",
        "response body:",
        "provider response body:",
        "raw provider response body:",
        "command payload:",
        "payload:",
    ] {
        if lower.contains(marker) {
            return true;
        }
    }

    false
}

fn sanitize_json_value_for_key(key: Option<&str>, value: serde_json::Value) -> serde_json::Value {
    if key.is_some_and(is_body_like_json_key) {
        return suppress_json_value(value);
    }

    if key.is_some_and(is_sensitive_json_key) {
        return serde_json::Value::String(REDACTED.to_string());
    }

    match value {
        serde_json::Value::String(string) => {
            serde_json::Value::String(sanitize_diagnostic_string(&string))
        }
        serde_json::Value::Array(values) => serde_json::Value::Array(
            values
                .into_iter()
                .map(|value| sanitize_json_value_for_key(None, value))
                .collect(),
        ),
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(child_key, value)| {
                    let sanitized = sanitize_json_value_for_key(Some(child_key.as_str()), value);
                    (child_key, sanitized)
                })
                .collect(),
        ),
        other => other,
    }
}

fn suppress_json_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(string) => {
            if string.len() > MAX_DIAGNOSTIC_STRING_LEN {
                serde_json::Value::String(REDACTED_LARGE_DIAGNOSTIC.to_string())
            } else {
                serde_json::Value::String(REDACTED_BODY.to_string())
            }
        }
        _ => serde_json::Value::String(REDACTED_BODY.to_string()),
    }
}

fn is_sensitive_json_key(key: &str) -> bool {
    let normalized = normalize_json_key(key);

    normalized.contains("authorization")
        || normalized.contains("authheader")
        || matches_sensitive_json_key_suffix(&normalized)
}

fn is_body_like_json_key(key: &str) -> bool {
    let normalized = normalize_json_key(key);

    matches!(
        normalized.as_str(),
        "body"
            | "payload"
            | "requestbody"
            | "responsebody"
            | "providerresponse"
            | "rawproviderresponse"
            | "commandpayload"
            | "request"
            | "response"
    )
}

fn normalize_json_key(key: &str) -> String {
    key.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

fn plain_text_sensitive_keys() -> &'static [&'static str] {
    &[
        "access_token",
        "api_key",
        "token",
        "worker_token",
        "provider_api_key",
        "hugging_face_key",
        "hf_token",
        "x-amz-signature",
        "x-amz-credential",
        "x-goog-signature",
    ]
}

fn matches_sensitive_json_key_suffix(normalized: &str) -> bool {
    [
        "apikey",
        "token",
        "accesstoken",
        "workertoken",
        "providerapikey",
        "huggingfacekey",
        "hftoken",
    ]
    .into_iter()
    .any(|suffix| normalized == suffix || normalized.ends_with(suffix))
}

fn redact_signed_url_tokens(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());

    for token in value.split_inclusive(char::is_whitespace) {
        let token_end = token.trim_end_matches(char::is_whitespace).len();
        let (content, suffix) = token.split_at(token_end);
        sanitized.push_str(&redact_signed_url_token(content));
        sanitized.push_str(suffix);
    }

    if sanitized.is_empty() && !value.is_empty() {
        redact_signed_url_token(value)
    } else {
        sanitized
    }
}

fn redact_signed_url_token(token: &str) -> String {
    let trimmed_end = token.trim_end_matches(['.', ',', ';', ':', ')', ']', '}']);
    let trailing = &token[trimmed_end.len()..];

    if looks_like_signed_url(trimmed_end) {
        format!("{REDACTED_URL}{trailing}")
    } else {
        token.to_string()
    }
}

fn looks_like_signed_url(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    (lower.contains("http://") || lower.contains("https://"))
        && [
            "x-amz-",
            "x-goog-",
            "signature=",
            "x-amz-signature=",
            "x-goog-signature=",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
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
    let mut sanitized = value.to_string();
    let mut search_from = 0;

    while let Some(relative_start) = find_case_insensitive(&sanitized[search_from..], key) {
        let start = search_from + relative_start;
        let key_end = start + key.len();

        if has_identifier_prefix(&sanitized, start) || has_identifier_suffix(&sanitized, key_end) {
            search_from = key_end;
            continue;
        }

        let separator_index = skip_key_suffix_delimiters(&sanitized, key_end);
        let Some(separator) = sanitized[separator_index..].chars().next() else {
            break;
        };
        if separator != ':' && separator != '=' {
            search_from = key_end;
            continue;
        }

        let value_start = skip_spaces(&sanitized, separator_index + separator.len_utf8());
        let (secret_start, secret_end) = if let Some(quote) = sanitized[value_start..]
            .chars()
            .next()
            .filter(|character| matches!(character, '"' | '\''))
        {
            let quoted_start = value_start + quote.len_utf8();
            let quoted_end = sanitized[quoted_start..]
                .find(quote)
                .map(|offset| quoted_start + offset)
                .unwrap_or_else(|| find_key_value_end(&sanitized, quoted_start));
            (quoted_start, quoted_end)
        } else {
            (value_start, find_key_value_end(&sanitized, value_start))
        };

        if secret_start == secret_end {
            search_from = value_start;
            continue;
        }

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

fn has_identifier_suffix(value: &str, index: usize) -> bool {
    value[index..]
        .chars()
        .next()
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

fn skip_key_suffix_delimiters(value: &str, index: usize) -> usize {
    let mut current = index;
    while let Some(character) = value[current..].chars().next() {
        if character.is_ascii_whitespace() || matches!(character, '"' | '\'') {
            current += character.len_utf8();
            continue;
        }
        break;
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

fn find_key_value_end(value: &str, index: usize) -> usize {
    value[index..]
        .find(['&', ' ', '\t', '\n', '\r', ',', ';', ')', ']', '}'])
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
    use logforth::kv::{KeyOwned, ValueOwned};
    use logforth::record::{FilterCriteria, Level};
    use logforth::Filter;
    use logforth::Layout;
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
            message: "request failed for Bearer top-secret-token at https://example.com/download?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Signature=deadbeef&foo=bar".to_string(),
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
            "request failed for Bearer [REDACTED] at [REDACTED_URL]"
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
        assert!(!diagnostics.message.contains("deadbeef"));
        assert!(!diagnostics.message.contains("https://example.com/download"));
        assert!(!diagnostics
            .cause
            .contains("abcdefghijklmnopqrstuvwxyz123456"));
        assert!(!diagnostics.cause.contains("native-secret"));
    }

    #[test]
    fn error_diagnostics_redacts_colon_and_quoted_sensitive_key_forms() {
        let error = TestError::StructuredFailure {
            message: r#"api_key: secret-one "access_token":"secret-two" authorization: Bearer secret-three"#.to_string(),
            source: Some(Box::new(TestError::StructuredFailure {
                message: r#""token":"secret-four""#.to_string(),
                source: Some(Box::new(TestError::StructuredFailure {
                    message: r#"provider said access_token: secret-five"#.to_string(),
                    source: None,
                })),
            })),
        };

        let diagnostics = error_diagnostics(&error, "fallback");

        assert_eq!(
            diagnostics.message,
            r#"api_key: [REDACTED] "access_token":"[REDACTED]" authorization: [REDACTED]"#
        );
        assert_eq!(diagnostics.cause, "provider said access_token: [REDACTED]");
        assert_eq!(
            diagnostics.source_chain,
            vec![
                r#""token":"[REDACTED]""#,
                "provider said access_token: [REDACTED]",
            ]
        );
    }

    #[test]
    fn error_diagnostics_suppresses_body_like_and_oversized_strings() {
        let oversized = "x".repeat(5000);
        let error = TestError::StructuredFailure {
            message: oversized,
            source: Some(Box::new(TestError::StructuredFailure {
                message: r#"provider response body: {"status":"ok","items":[1,2,3],"nested":{"step":"run"}}"#.to_string(),
                source: Some(Box::new(TestError::StructuredFailure {
                    message: r#"request body: {"prompt":"draw","workflow":{"id":"wf_123"}}"#.to_string(),
                    source: None,
                })),
            })),
        };

        let diagnostics = error_diagnostics(&error, "fallback");

        assert_eq!(diagnostics.message, "[REDACTED_LARGE_DIAGNOSTIC]");
        assert_eq!(diagnostics.cause, "[REDACTED_BODY]");
        assert_eq!(
            diagnostics.source_chain,
            vec!["[REDACTED_BODY]", "[REDACTED_BODY]"]
        );
    }

    #[test]
    fn error_diagnostics_keeps_safe_bracketed_messages_visible() {
        let error = TestError::StructuredFailure {
            message: "[workspace invalid] missing endpoint".to_string(),
            source: Some(Box::new(TestError::StructuredFailure {
                message: "{workspace invalid}".to_string(),
                source: None,
            })),
        };

        let diagnostics = error_diagnostics(&error, "fallback");

        assert_eq!(diagnostics.message, "[workspace invalid] missing endpoint");
        assert_eq!(diagnostics.cause, "{workspace invalid}");
        assert_eq!(diagnostics.source_chain, vec!["{workspace invalid}"]);
    }

    #[test]
    fn error_diagnostics_suppresses_plain_text_body_markers() {
        let error = TestError::StructuredFailure {
            message: "request body: not json but sensitive".to_string(),
            source: Some(Box::new(TestError::StructuredFailure {
                message: "payload: plain text secret".to_string(),
                source: Some(Box::new(TestError::StructuredFailure {
                    message: "response body: not-json".to_string(),
                    source: None,
                })),
            })),
        };

        let diagnostics = error_diagnostics(&error, "fallback");

        assert_eq!(diagnostics.message, "[REDACTED_BODY]");
        assert_eq!(diagnostics.cause, "[REDACTED_BODY]");
        assert_eq!(
            diagnostics.source_chain,
            vec!["[REDACTED_BODY]", "[REDACTED_BODY]"]
        );
    }

    #[test]
    fn sanitizing_json_layout_redacts_payload_and_key_values() {
        let layout = SanitizingJsonLayout;
        let key_values = [
            (
                KeyOwned::new("payload"),
                ValueOwned::str("provider response body: not json but sensitive"),
            ),
            (
                KeyOwned::new("download_url"),
                ValueOwned::str(
                    "https://example.com/download?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Signature=deadbeef",
                ),
            ),
        ];
        let record = logforth::record::Record::builder()
            .level(Level::Info)
            .target_static("luma_forge_lib::diagnostics")
            .file_static("src/diagnostics.rs")
            .line(Some(123))
            .payload(format_args!("Authorization: Bearer top-secret-token"))
            .key_values(&key_values[..])
            .build();

        let formatted = layout.format(&record, &[]).expect("record should format");
        let json: serde_json::Value =
            serde_json::from_slice(&formatted).expect("formatted line should be valid json");

        assert_eq!(json["message"], "Authorization: [REDACTED]");
        assert_eq!(json["kvs"]["payload"], "[REDACTED_BODY]");
        assert_eq!(json["kvs"]["download_url"], "[REDACTED_URL]");
        assert!(!formatted
            .windows("top-secret-token".len())
            .any(|window| window == b"top-secret-token"));
        assert!(!formatted
            .windows("deadbeef".len())
            .any(|window| window == b"deadbeef"));
    }

    #[test]
    fn sanitizing_json_layout_redacts_nested_structured_key_values() {
        let layout = SanitizingJsonLayout;
        let request_value = ValueOwned::map([
            (
                KeyOwned::new("authorization"),
                ValueOwned::str("Basic secret"),
            ),
            (
                KeyOwned::new("nested"),
                ValueOwned::map([
                    (KeyOwned::new("api_key"), ValueOwned::str("abc")),
                    (KeyOwned::new("access_token"), ValueOwned::str("xyz")),
                ]),
            ),
            (
                KeyOwned::new("body"),
                ValueOwned::map([
                    (KeyOwned::new("prompt"), ValueOwned::str("draw")),
                    (
                        KeyOwned::new("workflow"),
                        ValueOwned::map([(KeyOwned::new("id"), ValueOwned::str("wf_123"))]),
                    ),
                ]),
            ),
            (
                KeyOwned::new("payload"),
                ValueOwned::list([ValueOwned::map([(
                    KeyOwned::new("token"),
                    ValueOwned::str("array-secret"),
                )])]),
            ),
            (
                KeyOwned::new("safe"),
                ValueOwned::str("Bearer top-secret-token"),
            ),
        ]);
        let key_values = [(KeyOwned::new("details"), request_value)];
        let record = logforth::record::Record::builder()
            .level(Level::Info)
            .target_static("luma_forge_lib::diagnostics")
            .file_static("src/diagnostics.rs")
            .line(Some(123))
            .payload(format_args!("structured request"))
            .key_values(&key_values[..])
            .build();

        let formatted = layout.format(&record, &[]).expect("record should format");
        let json: serde_json::Value =
            serde_json::from_slice(&formatted).expect("formatted line should be valid json");

        assert_eq!(json["kvs"]["details"]["authorization"], "[REDACTED]");
        assert_eq!(json["kvs"]["details"]["nested"]["api_key"], "[REDACTED]");
        assert_eq!(
            json["kvs"]["details"]["nested"]["access_token"],
            "[REDACTED]"
        );
        assert_eq!(json["kvs"]["details"]["body"], "[REDACTED_BODY]");
        assert_eq!(json["kvs"]["details"]["payload"], "[REDACTED_BODY]");
        assert_eq!(json["kvs"]["details"]["safe"], "Bearer [REDACTED]");
        assert!(!formatted
            .windows("Basic secret".len())
            .any(|window| window == b"Basic secret"));
        assert!(!formatted
            .windows("array-secret".len())
            .any(|window| window == b"array-secret"));
    }

    #[test]
    fn sanitizing_json_layout_redacts_colon_and_quoted_sensitive_key_forms() {
        let layout = SanitizingJsonLayout;
        let key_values = [(
            KeyOwned::new("details"),
            ValueOwned::str(r#"api_key: secret-one "token":"secret-two""#),
        )];
        let record = logforth::record::Record::builder()
            .level(Level::Info)
            .target_static("luma_forge_lib::diagnostics")
            .file_static("src/diagnostics.rs")
            .line(Some(123))
            .payload(format_args!(r#"authorization: Bearer secret-three"#))
            .key_values(&key_values[..])
            .build();

        let formatted = layout.format(&record, &[]).expect("record should format");
        let json: serde_json::Value =
            serde_json::from_slice(&formatted).expect("formatted line should be valid json");

        assert_eq!(json["message"], "authorization: [REDACTED]");
        assert_eq!(
            json["kvs"]["details"],
            r#"api_key: [REDACTED] "token":"[REDACTED]""#
        );
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
