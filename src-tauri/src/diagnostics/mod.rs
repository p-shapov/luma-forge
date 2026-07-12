use std::fmt::{self, Debug};
use std::path::Path;

use logforth::filter::FilterResult;
use logforth::record::{FilterCriteria, Level, LevelFilter};

pub use luma_diagnostics_macros::{diagnostic, DiagnosticDebug};

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

pub fn current_trace_uuid() -> Option<uuid::Uuid> {
    fastrace::collector::SpanContext::current_local_parent()
        .map(|context| uuid::Uuid::from_u128(context.trace_id.0))
}

pub trait DiagnosticValue: Debug {}

pub fn shown<T: DiagnosticValue>(value: &T) -> &T {
    value
}

pub struct Redacted;

impl Debug for Redacted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

pub enum Field<'a> {
    Shown(&'a dyn Debug),
    Redacted,
}

impl<'a> Field<'a> {
    pub fn shown<T: DiagnosticValue>(value: &'a T) -> Self {
        Self::Shown(value)
    }

    pub const fn redacted() -> Self {
        Self::Redacted
    }
}

pub struct Fields<'a>(&'a [(&'static str, Field<'a>)]);

impl<'a> Fields<'a> {
    pub const fn new(fields: &'a [(&'static str, Field<'a>)]) -> Self {
        Self(fields)
    }
}

impl Debug for Fields<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut map = formatter.debug_map();
        for (name, field) in self.0 {
            match field {
                Field::Shown(value) => map.entry(name, value),
                Field::Redacted => map.entry(name, &Redacted),
            };
        }
        map.finish()
    }
}

macro_rules! impl_diagnostic_value {
    ($($type:ty),+ $(,)?) => {
        $(impl DiagnosticValue for $type {})+
    };
}

impl_diagnostic_value!(
    bool,
    char,
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    f32,
    f64,
    str,
    String,
    (),
    uuid::Uuid,
    secrecy::SecretString,
    time::OffsetDateTime,
);

impl<T: DiagnosticValue + ?Sized> DiagnosticValue for &T {}
impl<T: DiagnosticValue + ?Sized> DiagnosticValue for &mut T {}
impl<T: DiagnosticValue> DiagnosticValue for Option<T> {}
impl<T: DiagnosticValue> DiagnosticValue for Vec<T> {}
impl<T: DiagnosticValue> DiagnosticValue for [T] {}
impl<T: DiagnosticValue, const N: usize> DiagnosticValue for [T; N] {}

macro_rules! impl_diagnostic_tuple {
    ($($type:ident),+) => {
        impl<$($type: DiagnosticValue),+> DiagnosticValue for ($($type,)+) {}
    };
}

impl_diagnostic_tuple!(A);
impl_diagnostic_tuple!(A, B);
impl_diagnostic_tuple!(A, B, C);
impl_diagnostic_tuple!(A, B, C, D);
impl_diagnostic_tuple!(A, B, C, D, E);
impl_diagnostic_tuple!(A, B, C, D, E, F);

#[cfg(test)]
mod tests;
