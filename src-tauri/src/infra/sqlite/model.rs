use time::{format_description, OffsetDateTime, UtcOffset};

use super::errors::SqliteInfraError;

const SQLITE_TIMESTAMP_FORMAT: &str = "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:9][offset_hour sign:mandatory]:[offset_minute]";

#[derive(Debug, Clone)]
pub struct PersistedWorkspace {
    pub id: String,
    pub workflow_id: String,
    pub workflow_version: String,
    pub state: String,
    pub runtime_kind: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct PersistedRunpodRuntime {
    pub workspace_id: String,
    pub datacenter_id: String,
    pub gpu_id: String,
    pub volume_size_gb: i64,
    pub network_volume_id: Option<String>,
    pub provisioner_pod_id: Option<String>,
    pub endpoint_id: Option<String>,
    pub template_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PersistedLifecycleOperation {
    pub id: String,
    pub workspace_id: String,
    pub operation_kind: String,
    pub state: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Default)]
pub struct PersistedLifecycleOperationFilter {
    pub workspace_id: Option<String>,
    pub states: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PersistedRunpodPayload {
    pub operation_id: String,
    pub step: String,
}

pub(crate) fn format_timestamp(
    value: OffsetDateTime,
    operation: &'static str,
    column: &'static str,
) -> Result<String, SqliteInfraError> {
    let format = timestamp_format(operation)?;

    value
        .to_offset(UtcOffset::UTC)
        .format(&format)
        .map_err(|error| SqliteInfraError::StatementFailed {
            operation,
            message: format!("{column} timestamp could not be formatted: {error}"),
        })
}

pub(crate) fn parse_timestamp(
    value: &str,
    operation: &'static str,
    column: &'static str,
) -> Result<OffsetDateTime, SqliteInfraError> {
    let format = timestamp_format(operation)?;

    OffsetDateTime::parse(value, &format).map_err(|error| SqliteInfraError::CorruptData {
        operation,
        message: format!("{column} timestamp is not RFC3339: {error}"),
    })
}

fn timestamp_format(
    operation: &'static str,
) -> Result<Vec<format_description::FormatItem<'static>>, SqliteInfraError> {
    format_description::parse(SQLITE_TIMESTAMP_FORMAT).map_err(|error| {
        SqliteInfraError::StatementFailed {
            operation,
            message: format!("sqlite timestamp format is invalid: {error}"),
        }
    })
}
