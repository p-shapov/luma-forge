use std::path::Path;

use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::MigratorTrait;

use super::{errors::SqliteInfraError, migrations::Migrator};

#[derive(Debug, Clone)]
pub struct SqliteInfraDatabase {
    connection: DatabaseConnection,
}

impl SqliteInfraDatabase {
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self, SqliteInfraError> {
        let path = path.as_ref().to_string_lossy();
        let url = format!("sqlite://{path}?mode=rwc");
        let connection = Database::connect(&url)
            .await
            .map_err(|error| SqliteInfraError::ConnectFailed {
                operation: "connect sqlite database",
                message: error.to_string(),
            })?;

        connection
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                "PRAGMA foreign_keys = ON",
            ))
            .await
            .map_err(|error| SqliteInfraError::StatementFailed {
                operation: "enable sqlite foreign keys",
                message: error.to_string(),
            })?;

        Migrator::up(&connection, None)
            .await
            .map_err(|error| SqliteInfraError::StatementFailed {
                operation: "run sqlite migrations",
                message: error.to_string(),
            })?;

        Ok(Self { connection })
    }

    pub fn connection(&self) -> &DatabaseConnection {
        &self.connection
    }
}
