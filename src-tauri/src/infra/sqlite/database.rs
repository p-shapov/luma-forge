use std::path::Path;

use sea_orm::{ConnectOptions, Database, DatabaseConnection};

use super::errors::SqliteInfraError;

const ENTITY_REGISTRY_PREFIX: &str =
    concat!(env!("CARGO_CRATE_NAME"), "::infra::sqlite::entities::*");

#[derive(Debug, Clone)]
pub struct SqliteInfraDatabase {
    connection: DatabaseConnection,
}

impl SqliteInfraDatabase {
    #[luma_diagnostics::diagnostic]
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self, SqliteInfraError> {
        let path = path.as_ref().to_string_lossy();
        let url = format!("sqlite://{path}?mode=rwc");
        let mut options = ConnectOptions::new(url);
        options.map_sqlx_sqlite_opts(|options| options.foreign_keys(true));

        let connection = Database::connect(options)
            .await
            .map_err(SqliteInfraError::connect_failed("connect sqlite database"))?;

        connection
            .get_schema_registry(ENTITY_REGISTRY_PREFIX)
            .sync(&connection)
            .await
            .map_err(SqliteInfraError::schema_mismatch("sync sqlite schema"))?;

        Ok(Self { connection })
    }

    pub fn connection(&self) -> &DatabaseConnection {
        &self.connection
    }
}
