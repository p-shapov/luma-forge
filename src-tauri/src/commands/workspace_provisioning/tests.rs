use crate::domain::{
    placement as domain_placement,
    provider_setup::GpuCloudProviderId,
    runtime as domain_runtime, workflow as domain_workflow,
    workspace::{
        Workspace, WorkspaceLifecycleState, WorkspaceProvisioningPhase,
        WorkspaceProvisioningProgress, WorkspaceProvisioningStatus,
    },
};

use super::*;

#[test]
fn maps_provisioning_result_to_command_response_without_secret_fields() {
    let response = WorkspaceProvisioningResponse::from(WorkspaceProvisioningResult {
        workspace: Workspace {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            id: "018f6a40-0000-7000-8000-000000000001".to_string(),
            name: "Workspace".to_string(),
            lifecycle_state: WorkspaceLifecycleState::Provisioning,
            placement_plan: domain_placement::PlacementPlan::Runpod {
                selected_datacenter_id: "EU-RO-1".to_string(),
                selected_gpu_id: "NVIDIA RTX 4090".to_string(),
                persistent_storage_volume_size_bytes: 85899345920,
                endpoint_keep_alive_seconds: 5,
                selected_workflow_preset: domain_workflow::WorkflowPreset {
                    id: "preset".to_string(),
                    version: "1".to_string(),
                    name: "Preset".to_string(),
                    workflow_execution_type: domain_workflow::WorkflowExecutionType::T2i,
                    required_base_volume_size_bytes: 85899345920,
                    runtime_contract: domain_workflow::RuntimeContractReference {
                        id: "comfyui-python312-cu121".to_string(),
                        version: "1.0.0".to_string(),
                    },
                    required_model_assets: vec![],
                    required_custom_nodes: vec![],
                },
            },
            resolved_runtime_image: runtime_snapshot(),
            persistent_storage_volume_snapshot: None,
            active_provisioning_pod_snapshot: None,
            serverless_endpoint_snapshot: None,
            last_provisioning_pod_snapshot: None,
            provider_provisioning_snapshot: None,
            environment_prepared_at: None,
            last_provisioning_failure: None,
        },
        progress: WorkspaceProvisioningProgress {
            status: WorkspaceProvisioningStatus::Running,
            phase: WorkspaceProvisioningPhase::CreatingVolume,
            percent: None,
            failure: None,
        },
    });
    let payload = serde_json::to_value(&response).expect("response should serialize");

    assert_eq!(
        payload["workspace"]["id"],
        "018f6a40-0000-7000-8000-000000000001"
    );
    assert!(payload.get("provider_api_key").is_none());
    assert!(payload.get("bearer_token").is_none());
}

fn runtime_snapshot() -> domain_runtime::ResolvedRuntimeImageSnapshot {
    domain_runtime::ResolvedRuntimeImageSnapshot {
        contract_id: "comfyui-python312-cu121".to_string(),
        contract_version: "1.0.0".to_string(),
        provisioner_image_ref: "ghcr.io/luma-forge/provisioner-worker@sha256:1111111111111111111111111111111111111111111111111111111111111111".to_string(),
        endpoint_image_ref: "ghcr.io/luma-forge/runpod-endpoint-worker@sha256:2222222222222222222222222222222222222222222222222222222222222222".to_string(),
    }
}
