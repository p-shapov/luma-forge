use serde::Serialize;

use super::{sanitization::sanitize_diagnostic_string, HasDiagnosticCode};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ErrorDiagnosticFrame {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ErrorDiagnostics {
    pub code: String,
    pub message: String,
    pub chain: Vec<ErrorDiagnosticFrame>,
}

pub fn error_diagnostics<E>(error: &E) -> ErrorDiagnostics
where
    E: HasDiagnosticCode + 'static,
{
    let message = sanitize_diagnostic_string(&error.to_string());
    ErrorDiagnostics {
        code: error.diagnostic_code().to_string(),
        message: message.clone(),
        chain: error_diagnostic_chain(error, message),
    }
}

pub fn error_diagnostics_log_json<E>(error: &E) -> String
where
    E: HasDiagnosticCode + 'static,
{
    let diagnostics = error_diagnostics(error);
    serde_json::to_string(&diagnostics).unwrap_or_else(|_| {
        r#"{"code":"diagnostics_serialization_failed","message":"error diagnostics serialization failed","chain":[]}"#.to_string()
    })
}

fn error_diagnostic_chain<E>(error: &E, message: String) -> Vec<ErrorDiagnosticFrame>
where
    E: HasDiagnosticCode + 'static,
{
    let mut chain = vec![ErrorDiagnosticFrame {
        code: error.diagnostic_code().to_string(),
        message,
    }];

    let mut source = error.diagnostic_source();
    while let Some(error) = source {
        let message = sanitize_diagnostic_string(&error.to_string());
        chain.push(ErrorDiagnosticFrame {
            code: error.diagnostic_code().to_string(),
            message,
        });
        source = error.diagnostic_source();
    }

    chain
}
