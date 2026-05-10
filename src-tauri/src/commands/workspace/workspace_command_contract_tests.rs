use crate::{
    shared_contracts::provider_contracts::GpuCloudProviderId as ApplicationGpuCloudProviderId,
    workspace::workspace_contracts, workspace::workspace_setup_contracts,
};

use super::*;

#[test]
fn maps_inventory_request_to_application_contract() {
    let request = workspace_setup_contracts::GetProviderInventoryRequest::try_from(
        GetProviderInventoryRequest {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        },
    )
    .expect("request should map");

    assert_eq!(
        request.gpu_cloud_provider_id,
        ApplicationGpuCloudProviderId::Runpod
    );
}

#[test]
fn maps_create_workspace_request_to_application_contract() {
    let placement_plan = PlacementPlan {
        selected_datacenter_id: "EU-RO-1".to_string(),
        selected_gpu_id: "NVIDIA RTX 4090".to_string(),
        persistent_storage_volume_size_bytes: 85899345920,
        selected_workflow_preset: WorkflowPreset {
            id: "preset".to_string(),
            version: "1".to_string(),
            name: "Preset".to_string(),
            workflow_execution_type: WorkflowExecutionType::T2i,
            required_base_volume_size_bytes: 85899345920,
            required_comfyui_source: ComfyUiRuntimeSource::Git {
                repository_url: "https://github.com/comfyanonymous/ComfyUI.git".to_string(),
                revision: "main".to_string(),
            },
            required_model_assets: vec![],
            required_custom_nodes: vec![],
        },
        selected_provisioning_profile: ProvisioningProfile::Runpod {
            id: "provisioning".to_string(),
            version: "1".to_string(),
            name: "Provisioning".to_string(),
            provisioner_worker_runtime: ProvisionerWorkerRuntime {
                provisioner_version: "1".to_string(),
                docker_image_ref: "ghcr.io/luma-forge/provisioner:1".to_string(),
                volume_mount_path: "/workspace".to_string(),
                container_disk_bytes: 1,
                compute_type: ProvisioningComputeType::Pod,
                status_endpoint: ProvisioningStatusEndpoint {
                    port: 8080,
                    protocol: "http".to_string(),
                    status_path: "/status".to_string(),
                },
            },
            gpu_cloud_provider_config: RunPodProvisioningProfileConfig {
                cloud_type: None,
                pod_template_id: None,
                network_volume_mount_path: "/workspace".to_string(),
                expose_http_ports: vec![8080],
                env: None,
            },
        },
        selected_endpoint_profile: EndpointProfile::Runpod {
            id: "endpoint".to_string(),
            version: "1".to_string(),
            name: "Endpoint".to_string(),
            workflow_execution_type: WorkflowExecutionType::T2i,
            endpoint_worker_runtime: EndpointWorkerRuntime {
                endpoint_worker_version: "1".to_string(),
                docker_image_ref: "ghcr.io/luma-forge/endpoint:1".to_string(),
                http_port: 8080,
                health_path: "/health".to_string(),
                invoke_path: "/invoke".to_string(),
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
    };

    let request =
        workspace_setup_contracts::CreateWorkspaceRequest::try_from(CreateWorkspaceRequest {
            workspace_id: "018f6a40-0000-7000-8000-000000000001".to_string(),
            name: "Workspace".to_string(),
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            placement_plan,
        })
        .expect("request should map");

    assert_eq!(
        request.gpu_cloud_provider_id,
        ApplicationGpuCloudProviderId::Runpod
    );
    assert_eq!(request.workspace_id, "018f6a40-0000-7000-8000-000000000001");
    assert_eq!(request.name, "Workspace");
}

#[test]
fn maps_workspace_response_to_command_contract() {
    let response =
        CreateWorkspaceResponse::from(workspace_setup_contracts::CreateWorkspaceResponse {
            workspace: workspace_contracts::Workspace {
                gpu_cloud_provider_id: ApplicationGpuCloudProviderId::Runpod,
                id: "018f6a40-0000-7000-8000-000000000001".to_string(),
                name: "Workspace".to_string(),
                lifecycle_state: workspace_contracts::WorkspaceLifecycleState::Draft,
                placement_plan: workspace_contracts::PlacementPlan {
                    selected_datacenter_id: "EU-RO-1".to_string(),
                    selected_gpu_id: "NVIDIA RTX 4090".to_string(),
                    persistent_storage_volume_size_bytes: 85899345920,
                    selected_workflow_preset: workspace_contracts::WorkflowPreset {
                        id: "preset".to_string(),
                        version: "1".to_string(),
                        name: "Preset".to_string(),
                        workflow_execution_type: workspace_contracts::WorkflowExecutionType::T2i,
                        required_base_volume_size_bytes: 85899345920,
                        required_comfyui_source: workspace_contracts::ComfyUiRuntimeSource::Git {
                            repository_url: "https://github.com/comfyanonymous/ComfyUI.git"
                                .to_string(),
                            revision: "main".to_string(),
                        },
                        required_model_assets: vec![],
                        required_custom_nodes: vec![],
                    },
                    selected_provisioning_profile:
                        workspace_contracts::ProvisioningProfile::Runpod {
                            id: "provisioning".to_string(),
                            version: "1".to_string(),
                            name: "Provisioning".to_string(),
                            provisioner_worker_runtime:
                                workspace_contracts::ProvisionerWorkerRuntime {
                                    provisioner_version: "1".to_string(),
                                    docker_image_ref: "ghcr.io/luma-forge/provisioner:1"
                                        .to_string(),
                                    volume_mount_path: "/workspace".to_string(),
                                    container_disk_bytes: 1,
                                    compute_type: workspace_contracts::ProvisioningComputeType::Pod,
                                    status_endpoint:
                                        workspace_contracts::ProvisioningStatusEndpoint {
                                            port: 8080,
                                            protocol: "http".to_string(),
                                            status_path: "/status".to_string(),
                                        },
                                },
                            gpu_cloud_provider_config:
                                crate::provider::runpod::RunPodProvisioningProfileConfig {
                                    cloud_type: None,
                                    pod_template_id: None,
                                    network_volume_mount_path: "/workspace".to_string(),
                                    expose_http_ports: vec![8080],
                                    env: None,
                                },
                        },
                    selected_endpoint_profile: workspace_contracts::EndpointProfile::Runpod {
                        id: "endpoint".to_string(),
                        version: "1".to_string(),
                        name: "Endpoint".to_string(),
                        workflow_execution_type: workspace_contracts::WorkflowExecutionType::T2i,
                        endpoint_worker_runtime: workspace_contracts::EndpointWorkerRuntime {
                            endpoint_worker_version: "1".to_string(),
                            docker_image_ref: "ghcr.io/luma-forge/endpoint:1".to_string(),
                            http_port: 8080,
                            health_path: "/health".to_string(),
                            invoke_path: "/invoke".to_string(),
                        },
                        gpu_cloud_provider_config:
                            crate::provider::runpod::RunPodEndpointProfileConfig {
                                endpoint_template_id: None,
                                container_disk_bytes: 1,
                                volume_mount_path: "/workspace".to_string(),
                                env: None,
                                scaling: crate::provider::runpod::RunPodServerlessScalingConfig {
                                    min_workers: 0,
                                    max_workers: 1,
                                    idle_timeout_seconds: 60,
                                    scaler_type: None,
                                    scaler_value: None,
                                },
                            },
                    },
                },
                persistent_storage_volume_snapshot: None,
                active_provisioning_pod_snapshot: None,
                serverless_endpoint_snapshot: None,
                last_provisioning_pod_snapshot: None,
                environment_prepared_at: None,
            },
        });

    assert_eq!(
        response.workspace.gpu_cloud_provider_id,
        GpuCloudProviderId::Runpod
    );
    assert_eq!(
        response.workspace.lifecycle_state,
        WorkspaceLifecycleState::Draft
    );
}
