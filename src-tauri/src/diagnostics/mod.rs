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
mod tests;
