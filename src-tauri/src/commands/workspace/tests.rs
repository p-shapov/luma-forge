use crate::{
    domain::{
        placement as domain_placement,
        provider_setup::GpuCloudProviderId as DomainGpuCloudProviderId, runtime as domain_runtime,
        workflow as domain_workflow, workspace as domain_workspace,
    },
    workspace_setup::contracts::CreateWorkspaceInput,
};

use super::*;

#[test]
fn maps_inventory_request_to_domain_provider_id() {
    let request = GetProviderPlacementOptionsRequest {
        gpu_cloud_provider_id: DomainGpuCloudProviderId::Runpod,
    };

    assert_eq!(
        request.gpu_cloud_provider_id,
        DomainGpuCloudProviderId::Runpod
    );
}

#[test]
fn maps_provider_placement_options_response_to_command_contract() {
    let response = GetProviderPlacementOptionsResponse::from(
        crate::workspace_setup::contracts::ProviderPlacementOptions {
            provider_inventory: crate::domain::provider_inventory::ProviderInventory {
                gpu_cloud_provider_id: DomainGpuCloudProviderId::Runpod,
                fetched_at: "2026-05-08T00:00:00Z".to_string(),
                max_persistent_storage_volume_size_bytes: Some(100),
                datacenters: vec![],
            },
            placement_capabilities: domain_placement::ProviderPlacementCapabilities::runpod(),
        },
    );

    assert_eq!(
        response.provider_inventory.gpu_cloud_provider_id,
        DomainGpuCloudProviderId::Runpod
    );
    assert!(matches!(
        response.placement_capabilities.endpoint_keep_alive,
        domain_placement::EndpointKeepAliveCapability::Supported { .. }
    ));
}

fn command_placement_plan() -> domain_placement::PlacementPlan {
    domain_placement::PlacementPlan::Runpod {
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
        value["selected_workflow_preset"]["runtime_contract"]["id"],
        "comfyui-python312-cu121"
    );
    assert_eq!(
        value["selected_workflow_preset"]["runtime_contract"]["version"],
        "1.0.0"
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
        resolved_runtime_image: runtime_snapshot(),
        persistent_storage_volume_snapshot: None,
        active_provisioning_pod_snapshot: None,
        serverless_endpoint_snapshot: None,
        last_provisioning_pod_snapshot: None,
        provider_provisioning_snapshot: None,
        environment_prepared_at: None,
        last_provisioning_failure: None,
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

fn runtime_snapshot() -> domain_runtime::ResolvedRuntimeImageSnapshot {
    domain_runtime::ResolvedRuntimeImageSnapshot {
        contract_id: "comfyui-python312-cu121".to_string(),
        contract_version: "1.0.0".to_string(),
        provisioner_image_ref: "ghcr.io/luma-forge/provisioner-worker@sha256:1111111111111111111111111111111111111111111111111111111111111111".to_string(),
        endpoint_image_ref: "ghcr.io/luma-forge/runpod-endpoint-worker@sha256:2222222222222222222222222222222222222222222222222222222222222222".to_string(),
    }
}
