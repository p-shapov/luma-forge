use std::path::Path;

use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};

use super::{errors::WorkspaceCatalogError, schema};

#[derive(Debug, Clone)]
pub struct SqliteWorkspaceCatalogRepository {
    #[allow(dead_code)]
    pool: SqlitePool,
}

impl SqliteWorkspaceCatalogRepository {
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self, WorkspaceCatalogError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(options)
            .await
            .map_err(|_| WorkspaceCatalogError::StorageUnavailable)?;

        schema::bootstrap(&pool).await?;

        Ok(Self { pool })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use sqlx::Row;

    use super::*;

    fn catalog_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("luma-forge-{name}-{nonce}.sqlite"))
    }

    async fn table_exists(pool: &SqlitePool, table_name: &str) -> bool {
        let row = sqlx::query("SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?1")
            .bind(table_name)
            .fetch_optional(pool)
            .await
            .expect("sqlite_master query should succeed");

        row.is_some()
    }

    #[tokio::test]
    async fn connect_creates_schema() {
        let path = catalog_path("schema");

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should create schema");

        assert!(table_exists(&repository.pool, "metadata").await);
        assert!(table_exists(&repository.pool, "workspaces").await);

        let version = sqlx::query("SELECT value FROM metadata WHERE key = ?1")
            .bind("workspace_catalog_schema_version")
            .fetch_one(&repository.pool)
            .await
            .expect("metadata version should exist")
            .get::<String, _>("value");

        assert_eq!(version, "1");

        drop(repository);
        let _ = fs::remove_file(path);
    }
}
