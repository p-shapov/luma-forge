use std::path::Path;

mod error_details;
mod filtering;
mod json_layout;
mod sanitization;
mod tracing;

pub use error_details::{
    error_diagnostics, error_diagnostics_log_json, ErrorDiagnosticFrame, ErrorDiagnostics,
};
use filtering::AppLogFilter;
use json_layout::SanitizingJsonLayout;
pub use tracing::{current_trace_id, trace_id_from_context, trace_id_from_span};

#[derive(Debug, thiserror::Error, luma_diagnostic::DiagnosticCode)]
pub enum DiagnosticsInitializationError {
    #[error("native diagnostics could not be initialized: {message}")]
    SetupFailed { message: String },
}

pub trait HasDiagnosticCode: std::error::Error {
    fn diagnostic_code(&self) -> &'static str;

    fn diagnostic_source(&self) -> Option<&dyn HasDiagnosticCode> {
        None
    }
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

#[cfg(test)]
mod tests;
