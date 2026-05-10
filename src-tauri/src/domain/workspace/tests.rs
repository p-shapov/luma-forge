use crate::domain::{
    placement::PlacementPlan,
    profiles::{
        EndpointProfile, EndpointWorkerRuntime, ProvisionerWorkerRuntime, ProvisioningComputeType,
        ProvisioningProfile, ProvisioningStatusEndpoint, RunPodEndpointProfileConfig,
        RunPodProvisioningProfileConfig, RunPodServerlessScalingConfig,
    },
    provider_setup::GpuCloudProviderId,
    workflow::{ComfyUiRuntimeSource, WorkflowExecutionType, WorkflowPreset},
};

use super::{Workspace, WorkspaceLifecycleState, WorkspaceValidationError};

fn placement_plan() -> PlacementPlan {
    PlacementPlan::Runpod {
        selected_datacenter_id: "EU-RO-1".to_string(),
        selected_gpu_id: "NVIDIA RTX 4090".to_string(),
        persistent_storage_volume_size_bytes: 85899345920,
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
        selected_provisioning_profile: ProvisioningProfile::Runpod {
            id: "provisioning".to_string(),
            version: "1.0.0".to_string(),
            name: "Provisioning".to_string(),
            provisioner_worker_runtime: ProvisionerWorkerRuntime {
                provisioner_version: "1.0.0".to_string(),
                docker_image_ref: "ghcr.io/luma-forge/provisioner:1.0.0".to_string(),
                volume_mount_path: "/workspace".to_string(),
                container_disk_bytes: 1,
                compute_type: ProvisioningComputeType::Pod,
                status_endpoint: ProvisioningStatusEndpoint {
                    port: 8000,
                    protocol: "http".to_string(),
                    status_path: "/status".to_string(),
                },
            },
            gpu_cloud_provider_config: RunPodProvisioningProfileConfig {
                cloud_type: None,
                pod_template_id: None,
                network_volume_mount_path: "/workspace".to_string(),
                expose_http_ports: vec![8000],
                env: None,
            },
        },
        selected_endpoint_profile: EndpointProfile::Runpod {
            id: "endpoint".to_string(),
            version: "1.0.0".to_string(),
            name: "Endpoint".to_string(),
            workflow_execution_type: WorkflowExecutionType::T2i,
            endpoint_worker_runtime: EndpointWorkerRuntime {
                endpoint_worker_version: "1.0.0".to_string(),
                docker_image_ref: "ghcr.io/luma-forge/endpoint:1.0.0".to_string(),
                http_port: 8188,
                health_path: "/health".to_string(),
                invoke_path: "/prompt".to_string(),
            },
            gpu_cloud_provider_config: RunPodEndpointProfileConfig {
                endpoint_template_id: None,
                container_disk_bytes: 1,
                volume_mount_path: "/workspace".to_string(),
                env: None,
                scaling: RunPodServerlessScalingConfig {
                    min_workers: 0,
                    max_workers: 1,
                    idle_timeout_seconds: 60,
                    scaler_type: None,
                    scaler_value: None,
                },
            },
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
