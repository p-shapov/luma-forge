use crate::domain::workspace::{ProviderResourceStatus, Workspace};

pub(crate) fn is_workspace_ready(workspace: &Workspace) -> bool {
    workspace.environment_prepared_at.is_some()
        && workspace.active_provisioning_pod_snapshot.is_none()
        && workspace
            .persistent_storage_volume_snapshot
            .as_ref()
            .is_some_and(|snapshot| {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::workspace::ProviderResourceStatus,
        workspace_provisioning::test_support::ready_provisioning_workspace,
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
}
