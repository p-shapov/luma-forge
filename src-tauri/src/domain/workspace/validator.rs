use std::collections::HashSet;

use crate::domain::{
    provider_setup::GpuCloudProviderId,
    validation_error::{DomainValidationError, DomainValidationResult},
    validation_support::{is_blank, is_safe_absolute_posix_path},
};

use super::{
    PersistentStorageVolumeSnapshot, ProvisioningPodSnapshot, ServerlessEndpointSnapshot,
    Workspace, WorkspaceCatalog, WorkspaceLifecycleState,
};

pub fn validate_workspace_catalog(catalog: &WorkspaceCatalog) -> DomainValidationResult {
    let mut ids = HashSet::new();
    for workspace in &catalog.workspaces {
        if !ids.insert(workspace.id.as_str()) {
            return Err(DomainValidationError);
        }
        validate_workspace(workspace)?;
    }

    Ok(())
}

pub fn validate_workspace(workspace: &Workspace) -> DomainValidationResult {
    if is_blank(&workspace.id)
        || is_blank(&workspace.name)
        || workspace.placement_plan.gpu_cloud_provider_id() != workspace.gpu_cloud_provider_id
        || workspace
            .environment_prepared_at
            .as_deref()
            .is_some_and(is_blank)
    {
        return Err(DomainValidationError);
    }

    if matches!(workspace.lifecycle_state, WorkspaceLifecycleState::Draft)
        && (workspace.persistent_storage_volume_snapshot.is_some()
            || workspace.active_provisioning_pod_snapshot.is_some()
            || workspace.serverless_endpoint_snapshot.is_some()
            || workspace.last_provisioning_pod_snapshot.is_some()
            || workspace.environment_prepared_at.is_some())
    {
        return Err(DomainValidationError);
    }

    if let Some(snapshot) = &workspace.persistent_storage_volume_snapshot {
        validate_persistent_storage_volume_snapshot(workspace.gpu_cloud_provider_id, snapshot)?;
    }
    if let Some(snapshot) = &workspace.active_provisioning_pod_snapshot {
        validate_provisioning_pod_snapshot(workspace.gpu_cloud_provider_id, snapshot)?;
    }
    if let Some(snapshot) = &workspace.serverless_endpoint_snapshot {
        validate_serverless_endpoint_snapshot(workspace.gpu_cloud_provider_id, snapshot)?;
    }
    if let Some(snapshot) = &workspace.last_provisioning_pod_snapshot {
        validate_provisioning_pod_snapshot(workspace.gpu_cloud_provider_id, snapshot)?;
    }

    Ok(())
}

fn validate_persistent_storage_volume_snapshot(
    provider_id: GpuCloudProviderId,
    snapshot: &PersistentStorageVolumeSnapshot,
) -> DomainValidationResult {
    if snapshot.gpu_cloud_provider_id != provider_id
        || is_blank(&snapshot.provider_resource_id)
        || is_blank(&snapshot.datacenter_id)
        || snapshot.provisioned_size_bytes == 0
        || !is_safe_absolute_posix_path(&snapshot.mount_path)
    {
        return Err(DomainValidationError);
    }

    Ok(())
}

fn validate_provisioning_pod_snapshot(
    provider_id: GpuCloudProviderId,
    snapshot: &ProvisioningPodSnapshot,
) -> DomainValidationResult {
    if snapshot.gpu_cloud_provider_id != provider_id
        || is_blank(&snapshot.provider_resource_id)
        || is_blank(&snapshot.datacenter_id)
        || is_blank(&snapshot.selected_gpu_id)
        || is_blank(&snapshot.provisioning_profile_id)
        || is_blank(&snapshot.provisioner_status_url)
    {
        return Err(DomainValidationError);
    }

    Ok(())
}

fn validate_serverless_endpoint_snapshot(
    provider_id: GpuCloudProviderId,
    snapshot: &ServerlessEndpointSnapshot,
) -> DomainValidationResult {
    if snapshot.gpu_cloud_provider_id != provider_id
        || is_blank(&snapshot.provider_resource_id)
        || is_blank(&snapshot.datacenter_id)
        || is_blank(&snapshot.selected_gpu_id)
        || is_blank(&snapshot.endpoint_profile_id)
        || is_blank(&snapshot.endpoint_invoke_url)
    {
        return Err(DomainValidationError);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        placement::PlacementPlan,
        profiles::{
            EndpointProfile, EndpointWorkerRuntime, ProvisionerWorkerRuntime,
            ProvisioningComputeType, ProvisioningProfile, ProvisioningStatusEndpoint,
            RunPodEndpointProfileConfig, RunPodProvisioningProfileConfig,
            RunPodServerlessScalingConfig,
        },
        provider_setup::GpuCloudProviderId,
        workflow::{ComfyUiRuntimeSource, WorkflowExecutionType, WorkflowPreset},
        workspace::{ProviderResourceStatus, Workspace, WorkspaceLifecycleState},
    };

    use super::*;

    #[test]
    fn rejects_draft_workspace_with_resource_snapshot() {
        let mut workspace = valid_draft_workspace("workspace-1");
        workspace.persistent_storage_volume_snapshot = Some(PersistentStorageVolumeSnapshot {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            provider_resource_id: "volume-1".to_string(),
            datacenter_id: "EU-RO-1".to_string(),
            provider_resource_status: ProviderResourceStatus::Ready,
            provisioned_size_bytes: 1,
            mount_path: "/workspace".to_string(),
        });

        let error =
            validate_workspace(&workspace).expect_err("draft resource snapshot should fail");

        assert_eq!(error, DomainValidationError);
    }

    #[test]
    fn rejects_workspace_catalog_with_duplicate_ids() {
        let catalog = WorkspaceCatalog {
            workspaces: vec![
                valid_draft_workspace("workspace-1"),
                valid_draft_workspace("workspace-1"),
            ],
        };

        let error =
            validate_workspace_catalog(&catalog).expect_err("duplicate workspace id should fail");

        assert_eq!(error, DomainValidationError);
    }

    fn valid_draft_workspace(id: &str) -> Workspace {
        Workspace {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            id: id.to_string(),
            name: "Workspace".to_string(),
            lifecycle_state: WorkspaceLifecycleState::Draft,
            placement_plan: placement_plan(),
            persistent_storage_volume_snapshot: None,
            active_provisioning_pod_snapshot: None,
            serverless_endpoint_snapshot: None,
            last_provisioning_pod_snapshot: None,
            environment_prepared_at: None,
        }
    }

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
}
