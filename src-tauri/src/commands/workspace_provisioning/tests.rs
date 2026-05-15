use crate::domain::{
    placement as domain_placement,
    provider_setup::GpuCloudProviderId,
    workflow as domain_workflow,
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
                    required_comfyui_source: domain_workflow::ComfyUiRuntimeSource::Git {
                        repository_url: "https://github.com/comfyanonymous/ComfyUI.git".to_string(),
                        revision: "main".to_string(),
                    },
                    required_model_assets: vec![],
                    required_custom_nodes: vec![],
                },
            },
            persistent_storage_volume_snapshot: None,
            active_provisioning_pod_snapshot: None,
            serverless_endpoint_snapshot: None,
            last_provisioning_pod_snapshot: None,
            provider_provisioning_snapshot: None,
            environment_prepared_at: None,
        },
        progress: WorkspaceProvisioningProgress {
            status: WorkspaceProvisioningStatus::Running,
            phase: WorkspaceProvisioningPhase::CreatingVolume,
            percent: None,
            message: None,
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
