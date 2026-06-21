use std::path::Path;

use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};

use crate::{lifecycle_journal, workspace_catalog};

#[derive(Debug, thiserror::Error)]
pub enum SqliteNativeDatabaseError {
    #[error("sqlite error: {0}")]
    Sqlx(sqlx::Error),
    #[error("lifecycle journal error")]
    LifecycleJournal(lifecycle_journal::LifecycleJournalError),
    #[error("workspace catalog error")]
    WorkspaceCatalog(workspace_catalog::WorkspaceCatalogError),
}

impl From<sqlx::Error> for SqliteNativeDatabaseError {
    fn from(error: sqlx::Error) -> Self {
        Self::Sqlx(error)
    }
}

impl From<workspace_catalog::WorkspaceCatalogError> for SqliteNativeDatabaseError {
    fn from(error: workspace_catalog::WorkspaceCatalogError) -> Self {
        Self::WorkspaceCatalog(error)
    }
}

impl From<lifecycle_journal::LifecycleJournalError> for SqliteNativeDatabaseError {
    fn from(error: lifecycle_journal::LifecycleJournalError) -> Self {
        Self::LifecycleJournal(error)
    }
}

#[derive(Debug, Clone)]
pub struct SqliteNativeDatabase {
    pool: SqlitePool,
}

impl SqliteNativeDatabase {
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self, SqliteNativeDatabaseError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePool::connect_with(options).await?;

        workspace_catalog::sqlite::bootstrap(&pool).await?;
        lifecycle_journal::sqlite::bootstrap(&pool).await?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> SqlitePool {
        self.pool.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use sqlx::{Row, SqlitePool};

    use super::SqliteNativeDatabase;

    fn db_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("luma-forge-{name}-{nonce}.sqlite"))
    }

    async fn table_exists(pool: &SqlitePool, table_name: &str) -> bool {
        sqlx::query("SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?1")
            .bind(table_name)
            .fetch_optional(pool)
            .await
            .expect("sqlite_master query should succeed")
            .is_some()
    }

    #[tokio::test]
    async fn connect_bootstraps_workspace_and_lifecycle_tables() {
        let database = SqliteNativeDatabase::connect(db_path("native-bootstrap"))
            .await
            .expect("database should connect");
        let pool = database.pool();

        assert!(table_exists(&pool, "workspaces").await);
        assert!(table_exists(&pool, "lifecycle_operations").await);

        let row = sqlx::query("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await
            .expect("foreign key pragma should succeed");
        assert_eq!(row.get::<i64, _>("foreign_keys"), 1);
    }
}
