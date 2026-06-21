use std::{error::Error, path::Path};

use fastrace::{collector::SpanContext, Span};

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
    let appender = logforth::append::file::FileBuilder::new(logs_dir, current_log_filename())
        .layout(logforth::layout::JsonLayout::default())
        .build()
        .map_err(|error| DiagnosticsInitializationError::SetupFailed {
            message: error.to_string(),
        })?;

    let logger = logforth::bridge::log::LogBridge::new(
        logforth::core::builder()
            .dispatch(|dispatch| {
                dispatch
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
    log::set_max_level(log::LevelFilter::Trace);

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
        source_chain.push(source.to_string());
        leaf = source;
    }

    ErrorDiagnostics {
        code: serialized_error_code(error, fallback_code),
        message: error.to_string(),
        cause: leaf.to_string(),
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

fn current_log_filename() -> String {
    "luma-forge.log".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;

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
    fn current_log_filename_uses_single_stable_log_name() {
        assert_eq!(current_log_filename(), "luma-forge.log");
    }
}
