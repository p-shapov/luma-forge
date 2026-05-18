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
