use sqlx::{Row, SqlitePool};

use crate::lifecycle_journal::{
    errors::{schema_invalid_message, storage_unavailable_error},
    LifecycleJournalError,
};

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

struct ExpectedIndex {
    name: &'static str,
    column: &'static str,
    unique: bool,
    predicate: Option<&'static str>,
}

struct TableIndex {
    name: String,
    unique: bool,
    partial: bool,
}

pub async fn bootstrap(pool: &SqlitePool) -> Result<(), LifecycleJournalError> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS lifecycle_operations (
            id TEXT PRIMARY KEY NOT NULL,
            workspace_id TEXT NOT NULL,
            state TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            finished_at TEXT NULL
        )",
    )
    .execute(pool)
    .await
    .map_err(storage_unavailable_error)?;

    validate_table(
        pool,
        &[
            ExpectedColumn {
                name: "id",
                column_type: "TEXT",
                not_null: true,
                primary_key: true,
            },
            ExpectedColumn {
                name: "workspace_id",
                column_type: "TEXT",
                not_null: true,
                primary_key: false,
            },
            ExpectedColumn {
                name: "state",
                column_type: "TEXT",
                not_null: true,
                primary_key: false,
            },
            ExpectedColumn {
                name: "payload_json",
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
            ExpectedColumn {
                name: "finished_at",
                column_type: "TEXT",
                not_null: false,
                primary_key: false,
            },
        ],
    )
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_lifecycle_operations_workspace_id
         ON lifecycle_operations(workspace_id)",
    )
    .execute(pool)
    .await
    .map_err(storage_unavailable_error)?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_lifecycle_operations_state
         ON lifecycle_operations(state)",
    )
    .execute(pool)
    .await
    .map_err(storage_unavailable_error)?;

    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_lifecycle_operations_running_workspace_unique
         ON lifecycle_operations(workspace_id)
         WHERE state = 'running'",
    )
    .execute(pool)
    .await
    .map_err(storage_unavailable_error)?;

    validate_indexes(
        pool,
        &[
            ExpectedIndex {
                name: "idx_lifecycle_operations_workspace_id",
                column: "workspace_id",
                unique: false,
                predicate: None,
            },
            ExpectedIndex {
                name: "idx_lifecycle_operations_state",
                column: "state",
                unique: false,
                predicate: None,
            },
            ExpectedIndex {
                name: "idx_lifecycle_operations_running_workspace_unique",
                column: "workspace_id",
                unique: true,
                predicate: Some("WHERE state = 'running'"),
            },
        ],
    )
    .await?;

    Ok(())
}

async fn validate_table(
    pool: &SqlitePool,
    expected_columns: &[ExpectedColumn],
) -> Result<(), LifecycleJournalError> {
    let columns = table_columns(pool).await?;

    if columns.len() != expected_columns.len() {
        return Err(schema_invalid_message(format!(
            "expected {expected_columns_len} lifecycle operation columns, got {columns_len}",
            expected_columns_len = expected_columns.len(),
            columns_len = columns.len()
        )));
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
                && actual.primary_key == expected.primary_key
        });

    if matches {
        Ok(())
    } else {
        Err(schema_invalid_message(
            "lifecycle operation columns do not match expected columns",
        ))
    }
}

async fn table_columns(pool: &SqlitePool) -> Result<Vec<TableColumn>, LifecycleJournalError> {
    let rows = sqlx::query("PRAGMA table_info(lifecycle_operations)")
        .fetch_all(pool)
        .await
        .map_err(storage_unavailable_error)?;

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

async fn validate_indexes(
    pool: &SqlitePool,
    expected_indexes: &[ExpectedIndex],
) -> Result<(), LifecycleJournalError> {
    let indexes = table_indexes(pool).await?;

    for expected in expected_indexes {
        let Some(index) = indexes.iter().find(|index| index.name == expected.name) else {
            return Err(schema_invalid_message(format!(
                "index {index_name} is missing",
                index_name = expected.name
            )));
        };

        let columns = index_columns(pool, expected.name).await?;
        let sql = index_sql(pool, expected.name).await?;

        if index.unique != expected.unique
            || index.partial != expected.predicate.is_some()
            || columns.len() != 1
            || columns[0] != expected.column
            || !predicate_matches(sql.as_deref(), expected.predicate)
        {
            return Err(schema_invalid_message(format!(
                "index {index_name} does not match expected definition",
                index_name = expected.name
            )));
        };
    }

    Ok(())
}

async fn table_indexes(pool: &SqlitePool) -> Result<Vec<TableIndex>, LifecycleJournalError> {
    let rows = sqlx::query("PRAGMA index_list(lifecycle_operations)")
        .fetch_all(pool)
        .await
        .map_err(storage_unavailable_error)?;

    Ok(rows
        .into_iter()
        .map(|row| TableIndex {
            name: row.get("name"),
            unique: row.get::<i64, _>("unique") == 1,
            partial: row.get::<i64, _>("partial") == 1,
        })
        .collect())
}

async fn index_columns(
    pool: &SqlitePool,
    index_name: &str,
) -> Result<Vec<String>, LifecycleJournalError> {
    let rows = sqlx::query(&format!("PRAGMA index_info({index_name})"))
        .fetch_all(pool)
        .await
        .map_err(storage_unavailable_error)?;

    Ok(rows.into_iter().map(|row| row.get("name")).collect())
}

async fn index_sql(
    pool: &SqlitePool,
    index_name: &str,
) -> Result<Option<String>, LifecycleJournalError> {
    sqlx::query_scalar(
        "SELECT sql FROM sqlite_master
         WHERE type = 'index' AND tbl_name = 'lifecycle_operations' AND name = ?1",
    )
    .bind(index_name)
    .fetch_optional(pool)
    .await
    .map_err(storage_unavailable_error)
}

fn predicate_matches(actual_sql: Option<&str>, expected_predicate: Option<&str>) -> bool {
    match (actual_sql, expected_predicate) {
        (_, None) => true,
        (Some(actual_sql), Some(expected_predicate)) => normalized_where_tail(actual_sql)
            .is_some_and(|actual_predicate| {
                let expected_predicate = normalize_sql(expected_predicate);
                actual_predicate == expected_predicate
                    || actual_predicate == uppercase_where(&expected_predicate)
            }),
        _ => false,
    }
}

fn normalized_where_tail(sql: &str) -> Option<String> {
    let normalized = normalize_sql(sql);
    normalized
        .find("WHERE")
        .or_else(|| normalized.find("where"))
        .map(|start| normalized[start..].trim_end_matches(';').to_string())
}

fn normalize_sql(sql: &str) -> String {
    sql.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn uppercase_where(predicate: &str) -> String {
    predicate.replacen("where", "WHERE", 1)
}
