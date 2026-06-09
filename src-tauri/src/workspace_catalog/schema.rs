use sqlx::{Executor, Row, SqlitePool};

use super::errors::WorkspaceCatalogError;

const SCHEMA_VERSION_KEY: &str = "workspace_catalog_schema_version";
const SCHEMA_VERSION: &str = "1";

struct ExpectedColumn {
    name: &'static str,
    column_type: &'static str,
    not_null: bool,
    primary_key_position: i64,
}

struct TableColumn {
    name: String,
    column_type: String,
    not_null: bool,
    primary_key_position: i64,
}

struct ExpectedIndex {
    name: &'static str,
    column: &'static str,
}

struct IndexMetadata {
    unique: bool,
    partial: bool,
}

struct IndexKeyColumn {
    name: String,
    desc: bool,
    collation: Option<String>,
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

    let workspaces_existed = table_exists(pool, "workspaces").await?;

    pool.execute(
        "CREATE TABLE IF NOT EXISTS workspaces (
            id TEXT NOT NULL PRIMARY KEY,
            runtime_type TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            state TEXT NOT NULL,
            state_reason TEXT,
            workflow_preset_json TEXT NOT NULL,
            runtime_json TEXT NOT NULL,
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
                primary_key_position: 1,
            },
            ExpectedColumn {
                name: "value",
                column_type: "TEXT",
                not_null: true,
                primary_key_position: 0,
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
                not_null: true,
                primary_key_position: 1,
            },
            ExpectedColumn {
                name: "runtime_type",
                column_type: "TEXT",
                not_null: true,
                primary_key_position: 0,
            },
            ExpectedColumn {
                name: "provider_id",
                column_type: "TEXT",
                not_null: true,
                primary_key_position: 0,
            },
            ExpectedColumn {
                name: "state",
                column_type: "TEXT",
                not_null: true,
                primary_key_position: 0,
            },
            ExpectedColumn {
                name: "state_reason",
                column_type: "TEXT",
                not_null: false,
                primary_key_position: 0,
            },
            ExpectedColumn {
                name: "workflow_preset_json",
                column_type: "TEXT",
                not_null: true,
                primary_key_position: 0,
            },
            ExpectedColumn {
                name: "runtime_json",
                column_type: "TEXT",
                not_null: true,
                primary_key_position: 0,
            },
            ExpectedColumn {
                name: "created_at",
                column_type: "TEXT",
                not_null: true,
                primary_key_position: 0,
            },
            ExpectedColumn {
                name: "updated_at",
                column_type: "TEXT",
                not_null: true,
                primary_key_position: 0,
            },
        ],
    )
    .await?;

    let expected_indexes = [
        ExpectedIndex {
            name: "idx_workspaces_runtime_type",
            column: "runtime_type",
        },
        ExpectedIndex {
            name: "idx_workspaces_provider_id",
            column: "provider_id",
        },
        ExpectedIndex {
            name: "idx_workspaces_state",
            column: "state",
        },
    ];

    if workspaces_existed {
        validate_indexes(pool, "workspaces", &expected_indexes).await?;
    } else {
        pool.execute(
            "CREATE INDEX IF NOT EXISTS idx_workspaces_runtime_type ON workspaces (runtime_type)",
        )
        .await
        .map_err(|_| WorkspaceCatalogError::MigrationFailed)?;
        pool.execute(
            "CREATE INDEX IF NOT EXISTS idx_workspaces_provider_id ON workspaces (provider_id)",
        )
        .await
        .map_err(|_| WorkspaceCatalogError::MigrationFailed)?;
        pool.execute("CREATE INDEX IF NOT EXISTS idx_workspaces_state ON workspaces (state)")
            .await
            .map_err(|_| WorkspaceCatalogError::MigrationFailed)?;

        validate_indexes(pool, "workspaces", &expected_indexes).await?;
    }

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

async fn table_exists(pool: &SqlitePool, table_name: &str) -> Result<bool, WorkspaceCatalogError> {
    let row = sqlx::query("SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?1")
        .bind(table_name)
        .fetch_optional(pool)
        .await
        .map_err(|_| WorkspaceCatalogError::MigrationFailed)?;

    Ok(row.is_some())
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
                && actual.not_null == expected.not_null
                && actual.primary_key_position == expected.primary_key_position
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
            primary_key_position: row.get("pk"),
        })
        .collect())
}

async fn validate_indexes(
    pool: &SqlitePool,
    table_name: &str,
    expected_indexes: &[ExpectedIndex],
) -> Result<(), WorkspaceCatalogError> {
    let mut actual_index_names = user_defined_index_names(pool, table_name).await?;
    let mut expected_index_names = expected_indexes
        .iter()
        .map(|index| index.name.to_string())
        .collect::<Vec<_>>();
    actual_index_names.sort();
    expected_index_names.sort();

    if actual_index_names != expected_index_names {
        return Err(WorkspaceCatalogError::SchemaMismatch);
    }

    for expected in expected_indexes {
        let row =
            sqlx::query("SELECT tbl_name FROM sqlite_master WHERE type = 'index' AND name = ?1")
                .bind(expected.name)
                .fetch_optional(pool)
                .await
                .map_err(|_| WorkspaceCatalogError::MigrationFailed)?
                .ok_or(WorkspaceCatalogError::SchemaMismatch)?;

        if row.get::<String, _>("tbl_name") != table_name {
            return Err(WorkspaceCatalogError::SchemaMismatch);
        }

        let metadata = index_metadata(pool, table_name, expected.name).await?;
        if metadata.unique || metadata.partial {
            return Err(WorkspaceCatalogError::SchemaMismatch);
        }

        let columns = index_key_columns(pool, expected.name).await?;
        if columns.len() != 1 {
            return Err(WorkspaceCatalogError::SchemaMismatch);
        }

        let column = &columns[0];
        if column.name != expected.column
            || column.desc
            || column
                .collation
                .as_deref()
                .is_some_and(|collation| !collation.eq_ignore_ascii_case("BINARY"))
        {
            return Err(WorkspaceCatalogError::SchemaMismatch);
        }
    }

    Ok(())
}

async fn user_defined_index_names(
    pool: &SqlitePool,
    table_name: &str,
) -> Result<Vec<String>, WorkspaceCatalogError> {
    let rows = sqlx::query(
        "SELECT name FROM sqlite_master
         WHERE type = 'index'
           AND tbl_name = ?1
           AND name NOT LIKE 'sqlite_autoindex%'",
    )
    .bind(table_name)
    .fetch_all(pool)
    .await
    .map_err(|_| WorkspaceCatalogError::MigrationFailed)?;

    Ok(rows.into_iter().map(|row| row.get("name")).collect())
}

async fn index_metadata(
    pool: &SqlitePool,
    table_name: &str,
    index_name: &str,
) -> Result<IndexMetadata, WorkspaceCatalogError> {
    let rows = sqlx::query(&format!("PRAGMA index_list({table_name})"))
        .fetch_all(pool)
        .await
        .map_err(|_| WorkspaceCatalogError::MigrationFailed)?;

    let row = rows
        .into_iter()
        .find(|row| row.get::<String, _>("name") == index_name)
        .ok_or(WorkspaceCatalogError::SchemaMismatch)?;

    Ok(IndexMetadata {
        unique: row.get::<i64, _>("unique") == 1,
        partial: row
            .try_get::<i64, _>("partial")
            .is_ok_and(|partial| partial == 1),
    })
}

async fn index_key_columns(
    pool: &SqlitePool,
    index_name: &str,
) -> Result<Vec<IndexKeyColumn>, WorkspaceCatalogError> {
    let rows = sqlx::query(&format!("PRAGMA index_xinfo({index_name})"))
        .fetch_all(pool)
        .await
        .map_err(|_| WorkspaceCatalogError::MigrationFailed)?;

    Ok(rows
        .into_iter()
        .filter(|row| row.get::<i64, _>("key") == 1)
        .map(|row| IndexKeyColumn {
            name: row.get("name"),
            desc: row.get::<i64, _>("desc") == 1,
            collation: row.try_get("coll").ok(),
        })
        .collect())
}
