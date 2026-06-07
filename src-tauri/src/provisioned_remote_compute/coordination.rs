use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

#[derive(Clone, Default)]
pub(crate) struct ProvisionedRemoteComputeCoordinator {
    in_flight: Arc<Mutex<HashSet<String>>>,
}

impl ProvisionedRemoteComputeCoordinator {
    pub(crate) fn try_enter(&self, workspace_id: &str) -> Option<ProvisionedRemoteComputeGuard> {
        let mut in_flight = self
            .in_flight
            .lock()
            .expect("coordinator lock should succeed");
        if !in_flight.insert(workspace_id.to_string()) {
            return None;
        }

        Some(ProvisionedRemoteComputeGuard {
            workspace_id: workspace_id.to_string(),
            in_flight: Arc::clone(&self.in_flight),
        })
    }
}

pub(crate) struct ProvisionedRemoteComputeGuard {
    workspace_id: String,
    in_flight: Arc<Mutex<HashSet<String>>>,
}

impl Drop for ProvisionedRemoteComputeGuard {
    fn drop(&mut self) {
        self.in_flight
            .lock()
            .expect("coordinator lock should succeed")
            .remove(&self.workspace_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_enter_rejects_duplicate_workspace_until_guard_drops() {
        let coordinator = ProvisionedRemoteComputeCoordinator::default();

        let first = coordinator.try_enter("workspace-1");
        assert!(first.is_some());
        assert!(coordinator.try_enter("workspace-1").is_none());

        drop(first);

        assert!(coordinator.try_enter("workspace-1").is_some());
    }
}
