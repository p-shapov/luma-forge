use crate::domain::workspace::{
    ProviderProvisioningSnapshot, ProviderResourceStatus, RunPodEndpointTemplateSnapshot, Workspace,
};

pub(crate) fn is_workspace_ready(workspace: &Workspace) -> bool {
    workspace.environment_prepared_at.is_some()
        && workspace.active_provisioning_pod_snapshot.is_none()
        && workspace
            .persistent_storage_volume_snapshot
            .as_ref()
            .is_some_and(|snapshot| {
                snapshot.provider_resource_status == ProviderResourceStatus::Ready
            })
        && has_ready_matching_endpoint_template(workspace)
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

fn runpod_template_snapshot(workspace: &Workspace) -> Option<RunPodEndpointTemplateSnapshot> {
    match &workspace.provider_provisioning_snapshot {
        Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot,
        }) => endpoint_template_snapshot.clone(),
        None => None,
    }
}

pub(crate) fn has_ready_matching_endpoint_template(workspace: &Workspace) -> bool {
    runpod_template_snapshot(workspace)
        .as_ref()
        .is_some_and(|snapshot| endpoint_template_matches_workspace(snapshot, workspace))
}

fn endpoint_template_matches_workspace(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::workspace::{
            ProviderProvisioningSnapshot, ProviderResourceStatus, RunPodEndpointTemplateSnapshot,
        },
        workspace_provisioning::test_support::{ready_provisioning_workspace, template},
    };

    #[test]
    fn is_workspace_ready_accepts_ready_or_running_serverless_endpoint() {
        for status in [
            ProviderResourceStatus::Ready,
            ProviderResourceStatus::Running,
        ] {
            let mut workspace = ready_provisioning_workspace();
            workspace
                .serverless_endpoint_snapshot
                .as_mut()
                .expect("endpoint")
                .provider_resource_status = status;

            assert!(is_workspace_ready(&workspace));
        }
    }

    #[test]
    fn is_workspace_ready_requires_matching_ready_template() {
        let mut workspace = ready_provisioning_workspace();
        workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot: Some(RunPodEndpointTemplateSnapshot {
                endpoint_worker_image_ref: "ghcr.io/luma-forge/other@sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string(),
                ..template(ProviderResourceStatus::Ready)
            }),
        });

        assert!(!is_workspace_ready(&workspace));
    }
}
