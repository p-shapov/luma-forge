use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{
    runpod::{RunpodLifecycleCleanupPayload, RunpodLifecycleProvisionPayload},
    workspace::WorkspaceId,
};

pub type LifecycleOperationId = String;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum LifecycleOperationPayload {
    Provision(LifecycleProvisionPayload),
    Cleanup(LifecycleCleanupPayload),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "runtime_type", rename_all = "snake_case")]
pub enum LifecycleProvisionPayload {
    Runpod(RunpodLifecycleProvisionPayload),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "runtime_type", rename_all = "snake_case")]
pub enum LifecycleCleanupPayload {
    Runpod(RunpodLifecycleCleanupPayload),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleOperation {
    pub operation_id: LifecycleOperationId,
    pub workspace_id: WorkspaceId,
    pub state: LifecycleOperationState,
    pub payload: Option<LifecycleOperationPayload>,
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
