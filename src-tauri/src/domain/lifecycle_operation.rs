use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{runpod::RunpodLifecycleOperationPayload, workspace::WorkspaceId};

pub type LifecycleOperationId = String;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "runtime_type", rename_all = "snake_case")]
pub enum LifecycleOperationPayload {
    Runpod(RunpodLifecycleOperationPayload),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleOperation {
    pub operation_id: LifecycleOperationId,
    pub workspace_id: WorkspaceId,
    pub state: LifecycleOperationState,
    pub payload: LifecycleOperationPayload,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub finished_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleOperationState {
    Running,
    Completed,
    Failed,
    Stale,
}
