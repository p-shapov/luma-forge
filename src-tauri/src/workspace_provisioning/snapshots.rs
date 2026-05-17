use crate::domain::workspace::{
    PersistentStorageVolumeSnapshot, ProviderProvisioningSnapshot, ProviderResourceStatus,
    ProvisioningPodSnapshot, RunPodEndpointTemplateSnapshot, ServerlessEndpointSnapshot, Workspace,
};

use super::{
    contracts::{
        EndpointTemplateObservation, NetworkVolumeObservation, ProvisioningPodObservation,
        ServerlessEndpointObservation,
    },
    WorkspaceProvisioningError,
};

pub(crate) fn persistent_storage_volume_snapshot(
    workspace: &Workspace,
    observation: NetworkVolumeObservation,
) -> PersistentStorageVolumeSnapshot {
    PersistentStorageVolumeSnapshot {
        gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
        provider_resource_id: observation.provider_resource_id,
        provider_resource_status: observation.provider_resource_status,
        mount_path: observation.mount_path,
    }
}

pub(crate) fn created_provisioning_pod_snapshot(
    workspace: &Workspace,
    observation: ProvisioningPodObservation,
) -> Result<ProvisioningPodSnapshot, WorkspaceProvisioningError> {
    Ok(ProvisioningPodSnapshot {
        gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
        provider_resource_id: observation.provider_resource_id,
        provider_resource_status: observation.provider_resource_status,
        provisioner_status_url: observation
            .provisioner_status_url
            .ok_or(WorkspaceProvisioningError::ProviderResponseInvalid)?,
    })
}

pub(crate) fn observed_provisioning_pod_snapshot(
    workspace: &Workspace,
    previous: &ProvisioningPodSnapshot,
    observation: ProvisioningPodObservation,
) -> ProvisioningPodSnapshot {
    ProvisioningPodSnapshot {
        gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
        provider_resource_id: observation.provider_resource_id,
        provider_resource_status: observation.provider_resource_status,
        provisioner_status_url: observation
            .provisioner_status_url
            .unwrap_or_else(|| previous.provisioner_status_url.clone()),
    }
}

pub(crate) fn runpod_template_provisioning_snapshot(
    observation: EndpointTemplateObservation,
) -> ProviderProvisioningSnapshot {
    ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(RunPodEndpointTemplateSnapshot {
            template_id: observation.template_id,
            endpoint_worker_image_ref: observation.endpoint_worker_image_ref,
            mount_path: observation.mount_path,
            provider_resource_status: observation.provider_resource_status,
        }),
    }
}

pub(crate) fn serverless_endpoint_snapshot(
    workspace: &Workspace,
    observation: ServerlessEndpointObservation,
) -> ServerlessEndpointSnapshot {
    ServerlessEndpointSnapshot {
        gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
        provider_resource_id: observation.provider_resource_id,
        provider_resource_status: observation.provider_resource_status,
        endpoint_invoke_url: observation.endpoint_invoke_url,
    }
}

pub(crate) fn runpod_template_snapshot(
    workspace: &Workspace,
) -> Option<RunPodEndpointTemplateSnapshot> {
    match &workspace.provider_provisioning_snapshot {
        Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot,
        }) => endpoint_template_snapshot.clone(),
        None => None,
    }
}

pub(crate) fn endpoint_template_matches_workspace(
    template: &RunPodEndpointTemplateSnapshot,
    workspace: &Workspace,
) -> bool {
    template.provider_resource_status == ProviderResourceStatus::Ready
        && template.endpoint_worker_image_ref == workspace.resolved_runtime_image.endpoint_image_ref
        && template.mount_path
            == workspace
                .persistent_storage_volume_snapshot
                .as_ref()
                .map(|volume| volume.mount_path.clone())
                .unwrap_or_default()
}

pub(crate) fn is_workspace_ready(workspace: &Workspace) -> bool {
    workspace.environment_prepared_at.is_some()
        && workspace.active_provisioning_pod_snapshot.is_none()
        && workspace
            .persistent_storage_volume_snapshot
            .as_ref()
            .is_some_and(|snapshot| {
                snapshot.provider_resource_status == ProviderResourceStatus::Ready
            })
        && runpod_template_snapshot(workspace)
            .as_ref()
            .is_some_and(|snapshot| endpoint_template_matches_workspace(snapshot, workspace))
        && workspace
            .serverless_endpoint_snapshot
            .as_ref()
            .is_some_and(|snapshot| {
                snapshot.provider_resource_status == ProviderResourceStatus::Ready
            })
}

pub(crate) fn is_terminal_provider_resource_status(status: &ProviderResourceStatus) -> bool {
    matches!(
        status,
        ProviderResourceStatus::Failed
            | ProviderResourceStatus::Terminated
            | ProviderResourceStatus::Unknown
    )
}
