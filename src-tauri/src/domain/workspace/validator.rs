use std::collections::HashSet;

use crate::domain::{
    error::{DomainValidationError, DomainValidationResult},
    provider_setup::GpuCloudProviderId,
    runtime::validator as runtime_validator,
    validation::{is_blank, is_safe_absolute_posix_path},
};

use super::{
    PersistentStorageVolumeSnapshot, ProviderProvisioningSnapshot, ProviderResourceStatus,
    ProvisioningPodSnapshot, RunPodEndpointTemplateSnapshot, ServerlessEndpointSnapshot, Workspace,
    WorkspaceCatalog, WorkspaceLifecycleState,
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
        || runtime_validator::validate_resolved_runtime_snapshot(&workspace.resolved_runtime_image)
            .is_err()
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
            || workspace.provider_provisioning_snapshot.is_some()
            || workspace.environment_prepared_at.is_some()
            || workspace.last_provisioning_failure.is_some())
    {
        return Err(DomainValidationError);
    }

    if !matches!(workspace.lifecycle_state, WorkspaceLifecycleState::Failed)
        && workspace.last_provisioning_failure.is_some()
    {
        return Err(DomainValidationError);
    }

    if matches!(workspace.lifecycle_state, WorkspaceLifecycleState::Ready)
        && !has_ready_provisioning_state(workspace)
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
    if let Some(snapshot) = &workspace.provider_provisioning_snapshot {
        validate_provider_provisioning_snapshot(workspace.gpu_cloud_provider_id, snapshot)?;
    }
    if workspace.serverless_endpoint_snapshot.is_some()
        && runpod_endpoint_template_snapshot(workspace).is_none()
    {
        return Err(DomainValidationError);
    }

    Ok(())
}

fn has_ready_provisioning_state(workspace: &Workspace) -> bool {
    workspace.active_provisioning_pod_snapshot.is_none()
        && workspace.environment_prepared_at.is_some()
        && workspace
            .persistent_storage_volume_snapshot
            .as_ref()
            .is_some_and(|snapshot| {
                snapshot.provider_resource_status == ProviderResourceStatus::Ready
            })
        && runpod_endpoint_template_snapshot(workspace).is_some_and(|snapshot| {
            snapshot.provider_resource_status == ProviderResourceStatus::Ready
        })
        && workspace
            .serverless_endpoint_snapshot
            .as_ref()
            .is_some_and(|snapshot| {
                matches!(
                    snapshot.provider_resource_status,
                    ProviderResourceStatus::Ready | ProviderResourceStatus::Running
                )
            })
}

fn runpod_endpoint_template_snapshot(
    workspace: &Workspace,
) -> Option<&RunPodEndpointTemplateSnapshot> {
    match &workspace.provider_provisioning_snapshot {
        Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot,
        }) => endpoint_template_snapshot.as_ref(),
        None => None,
    }
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
        || is_blank(&snapshot.endpoint_invoke_url)
    {
        return Err(DomainValidationError);
    }

    Ok(())
}

fn validate_provider_provisioning_snapshot(
    provider_id: GpuCloudProviderId,
    snapshot: &ProviderProvisioningSnapshot,
) -> DomainValidationResult {
    match (provider_id, snapshot) {
        (
            GpuCloudProviderId::Runpod,
            ProviderProvisioningSnapshot::Runpod {
                endpoint_template_snapshot,
            },
        ) => {
            if let Some(template_snapshot) = endpoint_template_snapshot {
                validate_runpod_endpoint_template_snapshot(template_snapshot)?;
            }
        }
    }

    Ok(())
}

fn validate_runpod_endpoint_template_snapshot(
    snapshot: &RunPodEndpointTemplateSnapshot,
) -> DomainValidationResult {
    if is_blank(&snapshot.template_id)
        || is_blank(&snapshot.endpoint_worker_image_ref)
        || !is_safe_absolute_posix_path(&snapshot.mount_path)
    {
        return Err(DomainValidationError);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        placement::PlacementPlan,
        provider_setup::GpuCloudProviderId,
        runtime::ResolvedRuntimeImageSnapshot,
        workflow::{RuntimeContractReference, WorkflowExecutionType, WorkflowPreset},
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
            resolved_runtime_image: runtime_snapshot(),
            persistent_storage_volume_snapshot: None,
            active_provisioning_pod_snapshot: None,
            serverless_endpoint_snapshot: None,
            last_provisioning_pod_snapshot: None,
            provider_provisioning_snapshot: None,
            environment_prepared_at: None,
            last_provisioning_failure: None,
        }
    }

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
                runtime_contract: RuntimeContractReference {
                    id: "comfyui-python312-cu121".to_string(),
                    version: "1.0.0".to_string(),
                },
                required_model_assets: vec![],
                required_custom_nodes: vec![],
            },
        }
    }

    fn runtime_snapshot() -> ResolvedRuntimeImageSnapshot {
        ResolvedRuntimeImageSnapshot {
            contract_id: "comfyui-python312-cu121".to_string(),
            contract_version: "1.0.0".to_string(),
            provisioner_image_ref: "ghcr.io/luma-forge/provisioner-worker@sha256:1111111111111111111111111111111111111111111111111111111111111111".to_string(),
            endpoint_image_ref: "ghcr.io/luma-forge/runpod-endpoint-worker@sha256:2222222222222222222222222222222222222222222222222222222222222222".to_string(),
        }
    }
}
