use std::path::Path;

use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};

use super::errors::SqliteInfraError;

#[derive(Debug, Clone)]
pub struct SqliteInfraDatabase {
    connection: DatabaseConnection,
}

impl SqliteInfraDatabase {
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self, SqliteInfraError> {
        let path = path.as_ref().to_string_lossy();
        let url = format!("sqlite://{path}?mode=rwc");
        let connection =
            Database::connect(&url)
                .await
                .map_err(|error| SqliteInfraError::ConnectFailed {
                    operation: "connect sqlite database",
                    message: error.to_string(),
                })?;

        connection
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "PRAGMA foreign_keys = ON",
            ))
            .await
            .map_err(|error| SqliteInfraError::StatementFailed {
                operation: "enable sqlite foreign keys",
                message: error.to_string(),
            })?;

        connection
            .get_schema_registry("luma_forge_lib::infra::sqlite::entities::*")
            .sync(&connection)
            .await
            .map_err(|error| SqliteInfraError::SchemaMismatch {
                operation: "sync sqlite schema",
                message: error.to_string(),
            })?;

        Ok(Self { connection })
    }

    pub fn connection(&self) -> &DatabaseConnection {
        &self.connection
    }
}
