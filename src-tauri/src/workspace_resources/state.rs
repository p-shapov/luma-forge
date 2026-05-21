use crate::domain::workspace::{ProviderResourceStatus, Workspace, WorkspaceLifecycleState};

pub(crate) fn reset_after_resource_cleanup(workspace: &mut Workspace) {
    workspace.lifecycle_state = WorkspaceLifecycleState::Draft;
    workspace.persistent_storage_volume_snapshot = None;
    workspace.active_provisioning_pod_snapshot = None;
    workspace.serverless_endpoint_snapshot = None;
    workspace.last_provisioning_pod_snapshot = None;
    workspace.provider_provisioning_snapshot = None;
    workspace.environment_prepared_at = None;
    workspace.last_provisioning_failure = None;
}

pub(crate) fn is_terminal_provider_resource_status(status: &ProviderResourceStatus) -> bool {
    matches!(
        status,
        ProviderResourceStatus::Failed
            | ProviderResourceStatus::Terminated
            | ProviderResourceStatus::Unknown
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::workspace::ProviderResourceStatus,
        workspace_provisioning::{failure, test_support::ready_provisioning_workspace},
    };

    #[test]
    fn reset_after_resource_cleanup_returns_workspace_to_clean_draft() {
        let mut workspace = ready_provisioning_workspace();
        workspace.lifecycle_state = WorkspaceLifecycleState::Failed;
        workspace.last_provisioning_failure = Some(failure::cancellation_cleanup_failed());

        reset_after_resource_cleanup(&mut workspace);

        assert_eq!(workspace.lifecycle_state, WorkspaceLifecycleState::Draft);
        assert_eq!(workspace.persistent_storage_volume_snapshot, None);
        assert_eq!(workspace.active_provisioning_pod_snapshot, None);
        assert_eq!(workspace.serverless_endpoint_snapshot, None);
        assert_eq!(workspace.last_provisioning_pod_snapshot, None);
        assert_eq!(workspace.provider_provisioning_snapshot, None);
        assert_eq!(workspace.environment_prepared_at, None);
        assert_eq!(workspace.last_provisioning_failure, None);
    }

    #[test]
    fn terminal_provider_resource_statuses_are_failed_terminated_or_unknown() {
        for status in [
            ProviderResourceStatus::Failed,
            ProviderResourceStatus::Terminated,
            ProviderResourceStatus::Unknown,
        ] {
            assert!(is_terminal_provider_resource_status(&status));
        }

        for status in [
            ProviderResourceStatus::Creating,
            ProviderResourceStatus::Running,
            ProviderResourceStatus::Ready,
        ] {
            assert!(!is_terminal_provider_resource_status(&status));
        }
    }
}
