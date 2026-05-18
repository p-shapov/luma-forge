use std::{future::Future, pin::Pin};

use crate::domain::workspace::Workspace;

use super::WorkspaceResourceError;

pub(crate) mod runpod;

pub(crate) type WorkspaceResourceSyncResult = Result<Option<Workspace>, WorkspaceResourceError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceResourceConfig {
    pub(crate) volume_mount_path: String,
}

pub(crate) trait WorkspaceResourceOperations: Send + Sync {
    fn sync_network_volume<'a>(
        &'a self,
        workspace: &'a mut Workspace,
        config: &'a WorkspaceResourceConfig,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceSyncResult> + Send + 'a>>;

    fn sync_provisioning_pod<'a>(
        &'a self,
        workspace: &'a mut Workspace,
        config: &'a WorkspaceResourceConfig,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceSyncResult> + Send + 'a>>;

    fn finish_provisioning_pod<'a>(
        &'a self,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceSyncResult> + Send + 'a>>;

    fn sync_serverless_endpoint<'a>(
        &'a self,
        workspace: &'a mut Workspace,
        config: &'a WorkspaceResourceConfig,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceSyncResult> + Send + 'a>>;

    fn cleanup_known_resources<'a>(
        &'a self,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceResourceError>> + Send + 'a>>;
}
