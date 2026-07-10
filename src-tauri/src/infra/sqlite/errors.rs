use sea_orm::DbErr;

#[derive(Debug, thiserror::Error)]
pub enum SqliteInfraError {
    #[error("sqlite connection failed during {operation}: {message}")]
    ConnectFailed {
        operation: &'static str,
        message: String,
    },
    #[error("sqlite schema mismatch during {operation}: {message}")]
    SchemaMismatch {
        operation: &'static str,
        message: String,
    },
}

impl SqliteInfraError {
    pub(crate) fn connect_failed(operation: &'static str) -> impl FnOnce(DbErr) -> Self {
        move |error| Self::ConnectFailed {
            operation,
            message: error.to_string(),
        }
    }

    pub(crate) fn schema_mismatch(operation: &'static str) -> impl FnOnce(DbErr) -> Self {
        move |error| Self::SchemaMismatch {
            operation,
            message: error.to_string(),
        }
    }
}
