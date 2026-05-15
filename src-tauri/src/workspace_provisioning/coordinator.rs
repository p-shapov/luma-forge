use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone, Default)]
pub struct WorkspaceProvisioningCoordinator {
    active_workspace_ids: Arc<Mutex<HashSet<String>>>,
}

impl WorkspaceProvisioningCoordinator {
    pub(crate) fn try_enter(&self, workspace_id: &str) -> Option<WorkspaceProvisioningGuard> {
        let mut active = self
            .active_workspace_ids
            .lock()
            .expect("workspace provisioning coordinator lock");
        if !active.insert(workspace_id.to_string()) {
            return None;
        }
        Some(WorkspaceProvisioningGuard {
            workspace_id: workspace_id.to_string(),
            active_workspace_ids: self.active_workspace_ids.clone(),
        })
    }
}

pub(crate) struct WorkspaceProvisioningGuard {
    workspace_id: String,
    active_workspace_ids: Arc<Mutex<HashSet<String>>>,
}

impl Drop for WorkspaceProvisioningGuard {
    fn drop(&mut self) {
        self.active_workspace_ids
            .lock()
            .expect("workspace provisioning coordinator lock")
            .remove(&self.workspace_id);
    }
}
