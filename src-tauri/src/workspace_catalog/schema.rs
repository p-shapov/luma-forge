use sqlx::{Executor, Row, SqlitePool};

use super::errors::WorkspaceCatalogError;

const SCHEMA_VERSION_KEY: &str = "workspace_catalog_schema_version";
const SCHEMA_VERSION: &str = "1";

struct ExpectedColumn {
    name: &'static str,
    column_type: &'static str,
    not_null: bool,
    primary_key: bool,
}

struct TableColumn {
    name: String,
    column_type: String,
    not_null: bool,
    primary_key: bool,
}

pub async fn bootstrap(pool: &SqlitePool) -> Result<(), WorkspaceCatalogError> {
    pool.execute(
        "CREATE TABLE IF NOT EXISTS metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
    )
    .await
    .map_err(|_| WorkspaceCatalogError::MigrationFailed)?;

    pool.execute(
        "CREATE TABLE IF NOT EXISTS workspaces (
            id TEXT PRIMARY KEY,
            workspace_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
    )
    .await
    .map_err(|_| WorkspaceCatalogError::MigrationFailed)?;

    validate_table(
        pool,
        "metadata",
        &[
            ExpectedColumn {
                name: "key",
                column_type: "TEXT",
                not_null: false,
                primary_key: true,
            },
            ExpectedColumn {
                name: "value",
                column_type: "TEXT",
                not_null: true,
                primary_key: false,
            },
        ],
    )
    .await?;

    validate_table(
        pool,
        "workspaces",
        &[
            ExpectedColumn {
                name: "id",
                column_type: "TEXT",
                not_null: false,
                primary_key: true,
            },
            ExpectedColumn {
                name: "workspace_json",
                column_type: "TEXT",
                not_null: true,
                primary_key: false,
            },
            ExpectedColumn {
                name: "created_at",
                column_type: "TEXT",
                not_null: true,
                primary_key: false,
            },
            ExpectedColumn {
                name: "updated_at",
                column_type: "TEXT",
                not_null: true,
                primary_key: false,
            },
        ],
    )
    .await?;

    sqlx::query("INSERT OR IGNORE INTO metadata (key, value) VALUES (?1, ?2)")
        .bind(SCHEMA_VERSION_KEY)
        .bind(SCHEMA_VERSION)
        .execute(pool)
        .await
        .map_err(|_| WorkspaceCatalogError::MigrationFailed)?;

    let version: Option<String> = sqlx::query_scalar("SELECT value FROM metadata WHERE key = ?1")
        .bind(SCHEMA_VERSION_KEY)
        .fetch_optional(pool)
        .await
        .map_err(|_| WorkspaceCatalogError::MigrationFailed)?;

    match version.as_deref() {
        Some(SCHEMA_VERSION) => Ok(()),
        _ => Err(WorkspaceCatalogError::SchemaMismatch),
    }
}

async fn validate_table(
    pool: &SqlitePool,
    table_name: &str,
    expected_columns: &[ExpectedColumn],
) -> Result<(), WorkspaceCatalogError> {
    let columns = table_columns(pool, table_name).await?;

    if columns.len() != expected_columns.len() {
        return Err(WorkspaceCatalogError::SchemaMismatch);
    }

    let matches = columns
        .iter()
        .zip(expected_columns.iter())
        .all(|(actual, expected)| {
            actual.name == expected.name
                && actual
                    .column_type
                    .eq_ignore_ascii_case(expected.column_type)
                && (!expected.not_null || actual.not_null)
                && actual.primary_key == expected.primary_key
        });

    if matches {
        Ok(())
    } else {
        Err(WorkspaceCatalogError::SchemaMismatch)
    }
}

async fn table_columns(
    pool: &SqlitePool,
    table_name: &str,
) -> Result<Vec<TableColumn>, WorkspaceCatalogError> {
    let rows = sqlx::query(&format!("PRAGMA table_info({table_name})"))
        .fetch_all(pool)
        .await
        .map_err(|_| WorkspaceCatalogError::MigrationFailed)?;

    Ok(rows
        .into_iter()
        .map(|row| TableColumn {
            name: row.get("name"),
            column_type: row.get("type"),
            not_null: row.get::<i64, _>("notnull") == 1,
            primary_key: row.get::<i64, _>("pk") > 0,
        })
        .collect())
}
