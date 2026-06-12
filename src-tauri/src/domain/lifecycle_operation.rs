use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub use super::provisioned_remote::RunpodLifecycleOperationPayload;

pub type LifecycleOperationId = String;
pub type WorkspaceId = String;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "runtime_type", rename_all = "snake_case")]
pub enum LifecycleOperationPayload {
    Runpod(RunpodLifecycleOperationPayload),
}

#[cfg(test)]
mod tests {
    use super::{
        LifecycleOperation, LifecycleOperationPayload, LifecycleOperationState,
        RunpodLifecycleOperationPayload,
    };
    use crate::domain::{
        provisioned_remote::ProviderApiError,
        provisioned_remote::{
            RunpodCleanupStep, RunpodDeleteStep, RunpodLifecycleError, RunpodProvisionStep,
        },
    };
    use serde_json::json;
    use time::OffsetDateTime;

    #[test]
    fn lifecycle_payload_serializes_operation_kind_inside_runtime_payload() {
        let payload =
            LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Provision {
                step: Some(RunpodProvisionStep::CreateNetworkVolume),
                error: Some(RunpodLifecycleError::RunpodApiFailed {
                    reason: ProviderApiError::RateLimited,
                }),
            });

        let json = serde_json::to_value(&payload).expect("payload json");

        assert_eq!(
            json,
            json!({
                    "runtime_type": "runpod",
                    "operation": "provision",
                    "step": "create_network_volume",
                    "error": {
                        "runpod_api_failed": {
                        "reason": "rate_limited"
                    }
                }
            })
        );
        let object = json.as_object().expect("payload should be object");
        assert!(!object.contains_key("workspace_id"));
        assert!(!object.contains_key("message"));
    }

    #[test]
    fn provision_step_serializes_create_template() {
        let step = RunpodProvisionStep::CreateTemplate;

        let json = serde_json::to_value(step).expect("step json");

        assert_eq!(json, json!("create_template"));
    }

    #[test]
    fn cleanup_step_serializes_delete_template() {
        let step = RunpodCleanupStep::DeleteTemplate;

        let json = serde_json::to_value(step).expect("step json");

        assert_eq!(json, json!("delete_template"));
    }

    #[test]
    fn delete_step_serializes_delete_template() {
        let step = RunpodDeleteStep::DeleteTemplate;

        let json = serde_json::to_value(step).expect("step json");

        assert_eq!(json, json!("delete_template"));
    }

    #[test]
    fn lifecycle_operation_state_has_no_cancelling_variant() {
        let states = [
            LifecycleOperationState::Running,
            LifecycleOperationState::Completed,
            LifecycleOperationState::Failed,
            LifecycleOperationState::Stale,
        ];

        let json = serde_json::to_value(states).expect("states json");

        assert_eq!(json, json!(["running", "completed", "failed", "stale"]));
    }

    #[test]
    fn lifecycle_operation_serializes_timestamps_as_rfc3339_strings() {
        let operation = LifecycleOperation {
            operation_id: "operation-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            state: LifecycleOperationState::Running,
            payload: LifecycleOperationPayload::Runpod(
                RunpodLifecycleOperationPayload::Provision {
                    step: None,
                    error: None,
                },
            ),
            created_at: OffsetDateTime::from_unix_timestamp(0).expect("valid timestamp"),
            updated_at: OffsetDateTime::from_unix_timestamp(1).expect("valid timestamp"),
            finished_at: None,
        };

        let json = serde_json::to_value(&operation).expect("operation json");

        assert_eq!(
            json,
            json!({
                "operation_id": "operation-1",
                "workspace_id": "workspace-1",
                "state": "running",
                "payload": {
                    "runtime_type": "runpod",
                    "operation": "provision",
                    "step": null,
                    "error": null
                },
                "created_at": "1970-01-01T00:00:00Z",
                "updated_at": "1970-01-01T00:00:01Z",
                "finished_at": null
            })
        );

        let round_tripped: LifecycleOperation =
            serde_json::from_value(json).expect("operation should deserialize");

        assert_eq!(round_tripped, operation);
    }
}
