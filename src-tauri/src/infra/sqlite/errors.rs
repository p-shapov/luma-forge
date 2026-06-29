#[derive(Debug, thiserror::Error)]
pub enum SqliteInfraError {
    #[error("sqlite connection failed during {operation}: {message}")]
    ConnectFailed {
        operation: &'static str,
        message: String,
    },
    #[error("sqlite statement failed during {operation}: {message}")]
    StatementFailed {
        operation: &'static str,
        message: String,
    },
    #[error("sqlite schema mismatch during {operation}: {message}")]
    SchemaMismatch {
        operation: &'static str,
        message: String,
    },
    #[error("corrupt sqlite data during {operation}: {message}")]
    CorruptData {
        operation: &'static str,
        message: String,
    },
}
