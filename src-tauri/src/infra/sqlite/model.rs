use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use super::errors::SqliteInfraError;

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
    value
        .format(&Rfc3339)
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
    OffsetDateTime::parse(value, &Rfc3339).map_err(|error| SqliteInfraError::CorruptData {
        operation,
        message: format!("{column} timestamp is not RFC3339: {error}"),
    })
}
