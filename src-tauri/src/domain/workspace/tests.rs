use crate::domain::{
    placement::PlacementPlan,
    provider_setup::GpuCloudProviderId,
    workflow::{ComfyUiRuntimeSource, WorkflowExecutionType, WorkflowPreset},
};

use super::{Workspace, WorkspaceLifecycleState, WorkspaceValidationError};

fn placement_plan() -> PlacementPlan {
    PlacementPlan::Runpod {
        selected_datacenter_id: "EU-RO-1".to_string(),
        selected_gpu_id: "NVIDIA RTX 4090".to_string(),
        persistent_storage_volume_size_bytes: 85899345920,
        endpoint_keep_alive_seconds: 5,
        selected_workflow_preset: WorkflowPreset {
            id: "preset".to_string(),
            version: "1.0.0".to_string(),
            name: "Preset".to_string(),
            workflow_execution_type: WorkflowExecutionType::T2i,
            required_base_volume_size_bytes: 85899345920,
            required_comfyui_source: ComfyUiRuntimeSource::Git {
                repository_url: "https://github.com/comfyanonymous/ComfyUI".to_string(),
                revision: "main".to_string(),
            },
            required_model_assets: vec![],
            required_custom_nodes: vec![],
        },
    }
}

#[test]
fn creates_draft_workspace_with_empty_resource_snapshots() {
    let workspace = Workspace::new_draft(
        GpuCloudProviderId::Runpod,
        "018f6a40-0000-7000-8000-000000000001".to_string(),
        "Workspace".to_string(),
        placement_plan(),
    )
    .expect("draft workspace should be valid");

    assert_eq!(workspace.gpu_cloud_provider_id, GpuCloudProviderId::Runpod);
    assert_eq!(workspace.lifecycle_state, WorkspaceLifecycleState::Draft);
    assert!(workspace.persistent_storage_volume_snapshot.is_none());
    assert!(workspace.active_provisioning_pod_snapshot.is_none());
    assert!(workspace.serverless_endpoint_snapshot.is_none());
    assert!(workspace.last_provisioning_pod_snapshot.is_none());
    assert!(workspace.provider_provisioning_snapshot.is_none());
    assert_eq!(workspace.environment_prepared_at, None);
}

#[test]
fn rejects_missing_identity_or_name() {
    let missing_id = Workspace::new_draft(
        GpuCloudProviderId::Runpod,
        " ".to_string(),
        "Workspace".to_string(),
        placement_plan(),
    )
    .expect_err("missing id should fail");
    let missing_name = Workspace::new_draft(
        GpuCloudProviderId::Runpod,
        "018f6a40-0000-7000-8000-000000000001".to_string(),
        " ".to_string(),
        placement_plan(),
    )
    .expect_err("missing name should fail");

    assert_eq!(missing_id, WorkspaceValidationError);
    assert_eq!(missing_name, WorkspaceValidationError);
}

#[test]
fn placement_plan_carries_runpod_provider_identity() {
    assert_eq!(
        placement_plan().gpu_cloud_provider_id(),
        GpuCloudProviderId::Runpod
    );
}
