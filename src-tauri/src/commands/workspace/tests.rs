use crate::{
    domain::{
        placement as domain_placement, profiles as domain_profiles,
        provider_setup::GpuCloudProviderId as DomainGpuCloudProviderId,
        workflow as domain_workflow, workspace as domain_workspace,
    },
    workspace_setup::contracts::CreateWorkspaceInput,
};

use super::*;

#[test]
fn maps_inventory_request_to_domain_provider_id() {
    let request = GetProviderInventoryRequest {
        gpu_cloud_provider_id: DomainGpuCloudProviderId::Runpod,
    };

    assert_eq!(
        request.gpu_cloud_provider_id,
        DomainGpuCloudProviderId::Runpod
    );
}

fn command_placement_plan() -> domain_placement::PlacementPlan {
    domain_placement::PlacementPlan::Runpod {
        selected_datacenter_id: "EU-RO-1".to_string(),
        selected_gpu_id: "NVIDIA RTX 4090".to_string(),
        persistent_storage_volume_size_bytes: 85899345920,
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
        selected_provisioning_profile: domain_profiles::ProvisioningProfile::Runpod {
            id: "provisioning".to_string(),
            version: "1".to_string(),
            name: "Provisioning".to_string(),
            provisioner_worker_runtime: domain_profiles::ProvisionerWorkerRuntime {
                provisioner_version: "1".to_string(),
                docker_image_ref: "ghcr.io/luma-forge/provisioner:1".to_string(),
                volume_mount_path: "/workspace".to_string(),
                container_disk_bytes: 1,
                compute_type: domain_profiles::ProvisioningComputeType::Pod,
                status_endpoint: domain_profiles::ProvisioningStatusEndpoint {
                    port: 8080,
                    protocol: "http".to_string(),
                    status_path: "/status".to_string(),
                },
            },
            gpu_cloud_provider_config: domain_profiles::RunPodProvisioningProfileConfig {
                cloud_type: None,
                pod_template_id: None,
                network_volume_mount_path: "/workspace".to_string(),
                expose_http_ports: vec![8080],
                env: None,
            },
        },
        selected_endpoint_profile: domain_profiles::EndpointProfile::Runpod {
            id: "endpoint".to_string(),
            version: "1".to_string(),
            name: "Endpoint".to_string(),
            workflow_execution_type: domain_workflow::WorkflowExecutionType::T2i,
            endpoint_worker_runtime: domain_profiles::EndpointWorkerRuntime {
                endpoint_worker_version: "1".to_string(),
                docker_image_ref: "ghcr.io/luma-forge/endpoint:1".to_string(),
                http_port: 8080,
                health_path: "/health".to_string(),
                invoke_path: "/invoke".to_string(),
            },
            gpu_cloud_provider_config: domain_profiles::RunPodEndpointProfileConfig {
                endpoint_template_id: None,
                container_disk_bytes: 1,
                volume_mount_path: "/workspace".to_string(),
                env: None,
                scaling: domain_profiles::RunPodServerlessScalingConfig {
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
fn maps_create_workspace_request_to_service_input() {
    let request = CreateWorkspaceInput::from(CreateWorkspaceRequest {
        workspace_id: "018f6a40-0000-7000-8000-000000000001".to_string(),
        name: "Workspace".to_string(),
        gpu_cloud_provider_id: DomainGpuCloudProviderId::Runpod,
        placement_plan: command_placement_plan(),
    });

    assert_eq!(
        request.gpu_cloud_provider_id,
        DomainGpuCloudProviderId::Runpod
    );
    assert_eq!(request.workspace_id, "018f6a40-0000-7000-8000-000000000001");
    assert_eq!(request.name, "Workspace");
    assert!(matches!(
        request.placement_plan,
        domain_placement::PlacementPlan::Runpod { .. }
    ));
}

#[test]
fn serializes_command_placement_plan_with_provider_tag() {
    let value = serde_json::to_value(command_placement_plan()).expect("plan should serialize");

    assert_eq!(value["gpu_cloud_provider_id"], "runpod");
    assert_eq!(
        value["selected_workflow_preset"]["required_comfyui_source"]["source_type"],
        "git"
    );
}

#[test]
fn maps_workspace_response_to_command_contract() {
    let response = CreateWorkspaceResponse::from(domain_workspace::Workspace {
        gpu_cloud_provider_id: DomainGpuCloudProviderId::Runpod,
        id: "018f6a40-0000-7000-8000-000000000001".to_string(),
        name: "Workspace".to_string(),
        lifecycle_state: domain_workspace::WorkspaceLifecycleState::Draft,
        placement_plan: command_placement_plan(),
        persistent_storage_volume_snapshot: None,
        active_provisioning_pod_snapshot: None,
        serverless_endpoint_snapshot: None,
        last_provisioning_pod_snapshot: None,
        environment_prepared_at: None,
    });

    assert_eq!(
        response.workspace.gpu_cloud_provider_id,
        DomainGpuCloudProviderId::Runpod
    );
    assert_eq!(
        response.workspace.lifecycle_state,
        domain_workspace::WorkspaceLifecycleState::Draft
    );
}
