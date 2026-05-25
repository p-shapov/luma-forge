use std::{future::Future, pin::Pin};

use thiserror::Error;

use crate::{
    domain::workspace::{Workspace, WorkspaceCatalog, WorkspaceLifecycleState},
    secrets::{AsyncHuggingFaceApiKeyStore, AsyncProviderKeyStore, AsyncProvisionerTokenStore},
    workspace_catalog::repository::{DeleteWorkspaceResult, WorkspaceCatalogRepository},
    workspace_provisioning::WorkspaceProvisioningCoordinator,
    workspace_resources::{WorkspaceResourceError, WorkspaceResourceService},
    workspace_setup::error::WorkspaceSetupError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceRemovalResult {
    pub(crate) workspace_catalog: WorkspaceCatalog,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub(crate) enum WorkspaceRemovalError {
    #[error("workspace not found")]
    WorkspaceNotFound,
    #[error("invalid workspace lifecycle")]
    InvalidWorkspaceLifecycle,
    #[error("workspace catalog unavailable")]
    WorkspaceCatalogUnavailable,
    #[error("workspace catalog storage unavailable")]
    WorkspaceCatalogStorageUnavailable,
    #[error("workspace catalog migration failed")]
    WorkspaceCatalogMigrationFailed,
    #[error("workspace catalog query failed")]
    WorkspaceCatalogQueryFailed,
    #[error("workspace catalog corrupt")]
    WorkspaceCatalogCorrupt,
    #[error("workspace catalog schema mismatch")]
    WorkspaceCatalogSchemaMismatch,
    #[error("provider setup is incomplete")]
    ProviderSetupIncomplete,
    #[error("provider api key unauthorized")]
    ProviderApiKeyUnauthorized,
    #[error("provider api unavailable")]
    ProviderApiUnavailable,
    #[error("provider rate limited")]
    ProviderRateLimited,
    #[error("provider request rejected")]
    ProviderRequestRejected,
    #[error("provider response invalid")]
    ProviderResponseInvalid,
    #[error("provider resource not found")]
    ProviderResourceNotFound,
    #[error("provider orphaned resources")]
    ProviderOrphanedResources,
    #[error("provider operation conflict")]
    ProviderOperationConflict,
    #[error("provider operation indeterminate")]
    ProviderOperationIndeterminate,
    #[error("hugging face api key setup is required")]
    HuggingFaceApiKeySetupRequired,
    #[error("secure keyring unavailable")]
    SecureKeyringUnavailable,
    #[error("provisioner worker token invalid")]
    ProvisionerWorkerTokenInvalid,
    #[error("resource cleanup failed")]
    CleanupFailed,
}

impl From<WorkspaceSetupError> for WorkspaceRemovalError {
    fn from(error: WorkspaceSetupError) -> Self {
        match error {
            WorkspaceSetupError::WorkspaceCatalogUnavailable => Self::WorkspaceCatalogUnavailable,
            WorkspaceSetupError::WorkspaceCatalogStorageUnavailable => {
                Self::WorkspaceCatalogStorageUnavailable
            }
            WorkspaceSetupError::WorkspaceCatalogMigrationFailed => {
                Self::WorkspaceCatalogMigrationFailed
            }
            WorkspaceSetupError::WorkspaceCatalogQueryFailed => Self::WorkspaceCatalogQueryFailed,
            WorkspaceSetupError::WorkspaceCatalogCorrupt => Self::WorkspaceCatalogCorrupt,
            WorkspaceSetupError::WorkspaceCatalogSchemaMismatch => {
                Self::WorkspaceCatalogSchemaMismatch
            }
            _ => Self::WorkspaceCatalogUnavailable,
        }
    }
}

impl From<WorkspaceResourceError> for WorkspaceRemovalError {
    fn from(error: WorkspaceResourceError) -> Self {
        match error {
            WorkspaceResourceError::WorkspaceCatalogUnavailable => {
                Self::WorkspaceCatalogUnavailable
            }
            WorkspaceResourceError::WorkspaceCatalogStorageUnavailable => {
                Self::WorkspaceCatalogStorageUnavailable
            }
            WorkspaceResourceError::WorkspaceCatalogMigrationFailed => {
                Self::WorkspaceCatalogMigrationFailed
            }
            WorkspaceResourceError::WorkspaceCatalogQueryFailed => {
                Self::WorkspaceCatalogQueryFailed
            }
            WorkspaceResourceError::WorkspaceCatalogCorrupt => Self::WorkspaceCatalogCorrupt,
            WorkspaceResourceError::WorkspaceCatalogSchemaMismatch => {
                Self::WorkspaceCatalogSchemaMismatch
            }
            WorkspaceResourceError::ProviderSetupIncomplete => Self::ProviderSetupIncomplete,
            WorkspaceResourceError::ProviderApiKeyUnauthorized => Self::ProviderApiKeyUnauthorized,
            WorkspaceResourceError::ProviderApiUnavailable => Self::ProviderApiUnavailable,
            WorkspaceResourceError::ProviderRateLimited => Self::ProviderRateLimited,
            WorkspaceResourceError::ProviderRequestRejected => Self::ProviderRequestRejected,
            WorkspaceResourceError::ProviderResponseInvalid => Self::ProviderResponseInvalid,
            WorkspaceResourceError::ProviderResourceNotFound => Self::ProviderResourceNotFound,
            WorkspaceResourceError::ProviderOrphanedResources => Self::ProviderOrphanedResources,
            WorkspaceResourceError::ProviderOperationConflict => Self::ProviderOperationConflict,
            WorkspaceResourceError::ProviderOperationIndeterminate => {
                Self::ProviderOperationIndeterminate
            }
            WorkspaceResourceError::HuggingFaceApiKeySetupRequired => {
                Self::HuggingFaceApiKeySetupRequired
            }
            WorkspaceResourceError::SecureKeyringUnavailable => Self::SecureKeyringUnavailable,
            WorkspaceResourceError::ProvisionerWorkerTokenInvalid => {
                Self::ProvisionerWorkerTokenInvalid
            }
            WorkspaceResourceError::CleanupFailed => Self::CleanupFailed,
        }
    }
}

pub(crate) trait WorkspaceRemovalResources {
    fn cleanup_known_resources<'a>(
        &'a self,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceResourceError>> + Send + 'a>>;
}

impl<S, W> WorkspaceRemovalResources for WorkspaceResourceService<S, W>
where
    S: AsyncHuggingFaceApiKeyStore + AsyncProviderKeyStore + AsyncProvisionerTokenStore,
    W: WorkspaceCatalogRepository,
{
    fn cleanup_known_resources<'a>(
        &'a self,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceResourceError>> + Send + 'a>> {
        Box::pin(
            async move { WorkspaceResourceService::cleanup_known_resources(self, workspace).await },
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) struct WorkspaceRemovalService<W, R> {
    workspace_catalog: W,
    resources: R,
    coordinator: WorkspaceProvisioningCoordinator,
}

impl<W, R> WorkspaceRemovalService<W, R> {
    pub(crate) fn new(
        workspace_catalog: W,
        resources: R,
        coordinator: WorkspaceProvisioningCoordinator,
    ) -> Self {
        Self {
            workspace_catalog,
            resources,
            coordinator,
        }
    }
}

impl<W, R> WorkspaceRemovalService<W, R>
where
    W: WorkspaceCatalogRepository,
    R: WorkspaceRemovalResources,
{
    pub(crate) async fn delete_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<WorkspaceRemovalResult, WorkspaceRemovalError> {
        let mut workspace = self
            .workspace_catalog
            .find_workspace_by_id(workspace_id)
            .await
            .map_err(WorkspaceRemovalError::from)?
            .ok_or(WorkspaceRemovalError::WorkspaceNotFound)?;

        if workspace.lifecycle_state == WorkspaceLifecycleState::Provisioning {
            return Err(WorkspaceRemovalError::InvalidWorkspaceLifecycle);
        }

        let Some(_guard) = self.coordinator.try_enter(workspace_id) else {
            return Err(WorkspaceRemovalError::ProviderOperationConflict);
        };

        if has_cleanup_metadata(&workspace) {
            self.resources
                .cleanup_known_resources(&mut workspace)
                .await
                .map_err(WorkspaceRemovalError::from)?;
        }

        match self
            .workspace_catalog
            .delete_workspace(workspace_id)
            .await
            .map_err(WorkspaceRemovalError::from)?
        {
            DeleteWorkspaceResult::Deleted => {}
            DeleteWorkspaceResult::NotFound => {
                return Err(WorkspaceRemovalError::WorkspaceNotFound)
            }
            DeleteWorkspaceResult::InvalidLifecycle => {
                return Err(WorkspaceRemovalError::InvalidWorkspaceLifecycle);
            }
        }
        let workspace_catalog = self
            .workspace_catalog
            .list_workspaces()
            .await
            .map_err(WorkspaceRemovalError::from)?;

        Ok(WorkspaceRemovalResult { workspace_catalog })
    }
}

fn has_cleanup_metadata(workspace: &Workspace) -> bool {
    workspace.persistent_storage_volume_snapshot.is_some()
        || workspace.active_provisioning_pod_snapshot.is_some()
        || workspace.serverless_endpoint_snapshot.is_some()
        || workspace.last_provisioning_pod_snapshot.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::workspace::{Workspace, WorkspaceCatalog, WorkspaceLifecycleState},
        workspace_catalog::repository::WorkspaceCatalogRepository,
        workspace_provisioning::{
            test_support::{ready_provisioning_workspace, volume, workspace as draft_workspace},
            WorkspaceProvisioningCoordinator,
        },
        workspace_resources::WorkspaceResourceError,
        workspace_setup::error::WorkspaceSetupError,
    };
    use std::{
        future::Future,
        pin::Pin,
        sync::{Arc, Mutex},
    };

    type CleanupCallback = Box<dyn Fn() + Send>;

    #[derive(Debug, Clone)]
    struct FakeWorkspaceCatalog {
        workspaces: Arc<Mutex<Vec<Workspace>>>,
        delete_error: Arc<Mutex<Option<WorkspaceSetupError>>>,
    }

    impl FakeWorkspaceCatalog {
        fn with_workspaces(workspaces: impl IntoIterator<Item = Workspace>) -> Self {
            Self {
                workspaces: Arc::new(Mutex::new(workspaces.into_iter().collect())),
                delete_error: Arc::new(Mutex::new(None)),
            }
        }

        fn missing() -> Self {
            Self::with_workspaces([])
        }

        fn push_delete_error(&self, error: WorkspaceSetupError) {
            *self.delete_error.lock().expect("fake delete error") = Some(error);
        }

        fn set_lifecycle_state(&self, lifecycle_state: WorkspaceLifecycleState) {
            let mut workspaces = self.workspaces.lock().expect("fake workspaces");
            let workspace = workspaces
                .first_mut()
                .expect("fake catalog should contain workspace");
            workspace.lifecycle_state = lifecycle_state;
        }

        fn stored_workspaces(&self) -> Vec<Workspace> {
            self.workspaces.lock().expect("fake workspaces").clone()
        }
    }

    impl WorkspaceCatalogRepository for FakeWorkspaceCatalog {
        fn list_workspaces<'a>(
            &'a self,
        ) -> Pin<Box<dyn Future<Output = Result<WorkspaceCatalog, WorkspaceSetupError>> + Send + 'a>>
        {
            Box::pin(async move {
                Ok(WorkspaceCatalog {
                    workspaces: self.workspaces.lock().expect("fake workspaces").clone(),
                })
            })
        }

        fn find_workspace_by_id<'a>(
            &'a self,
            id: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<Option<Workspace>, WorkspaceSetupError>> + Send + 'a>>
        {
            Box::pin(async move {
                Ok(self
                    .workspaces
                    .lock()
                    .expect("fake workspaces")
                    .iter()
                    .find(|workspace| workspace.id == id)
                    .cloned())
            })
        }

        fn insert_workspace<'a>(
            &'a self,
            workspace: &'a Workspace,
        ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>>
        {
            Box::pin(async move {
                self.workspaces
                    .lock()
                    .expect("fake workspaces")
                    .push(workspace.clone());
                Ok(workspace.clone())
            })
        }

        fn update_workspace<'a>(
            &'a self,
            workspace: &'a Workspace,
        ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>>
        {
            Box::pin(async move { Ok(workspace.clone()) })
        }

        fn delete_workspace<'a>(
            &'a self,
            id: &'a str,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<DeleteWorkspaceResult, WorkspaceSetupError>> + Send + 'a,
            >,
        > {
            Box::pin(async move {
                if let Some(error) = self.delete_error.lock().expect("fake delete error").take() {
                    return Err(error);
                }
                let mut workspaces = self.workspaces.lock().expect("fake workspaces");
                if workspaces.iter().any(|workspace| {
                    workspace.id == id
                        && workspace.lifecycle_state == WorkspaceLifecycleState::Provisioning
                }) {
                    return Ok(DeleteWorkspaceResult::InvalidLifecycle);
                }
                let index = workspaces.iter().position(|workspace| workspace.id == id);
                let Some(index) = index else {
                    return Ok(DeleteWorkspaceResult::NotFound);
                };
                workspaces.remove(index);
                Ok(DeleteWorkspaceResult::Deleted)
            })
        }
    }

    #[derive(Clone, Default)]
    struct FakeWorkspaceResources {
        calls: Arc<Mutex<Vec<String>>>,
        cleanup_result: Arc<Mutex<Option<Result<Workspace, WorkspaceResourceError>>>>,
        on_cleanup: Arc<Mutex<Option<CleanupCallback>>>,
    }

    impl FakeWorkspaceResources {
        fn calls(&self) -> Vec<String> {
            self.calls.lock().expect("fake resource calls").clone()
        }

        fn push_cleanup_result(&self, result: Result<Workspace, WorkspaceResourceError>) {
            *self.cleanup_result.lock().expect("fake cleanup result") = Some(result);
        }

        fn on_cleanup(&self, callback: impl Fn() + Send + 'static) {
            *self.on_cleanup.lock().expect("fake cleanup callback") = Some(Box::new(callback));
        }
    }

    impl WorkspaceRemovalResources for FakeWorkspaceResources {
        fn cleanup_known_resources<'a>(
            &'a self,
            workspace: &'a mut Workspace,
        ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceResourceError>> + Send + 'a>>
        {
            Box::pin(async move {
                self.calls
                    .lock()
                    .expect("fake resource calls")
                    .push(workspace.id.clone());
                if let Some(callback) = self
                    .on_cleanup
                    .lock()
                    .expect("fake cleanup callback")
                    .take()
                {
                    callback();
                }
                self.cleanup_result
                    .lock()
                    .expect("fake cleanup result")
                    .take()
                    .unwrap_or_else(|| Ok(workspace.clone()))
            })
        }
    }

    #[tokio::test]
    async fn delete_draft_workspace_skips_cleanup_and_returns_updated_catalog() {
        let workspace = draft_workspace();
        let catalog = FakeWorkspaceCatalog::with_workspaces([workspace.clone()]);
        let resources = FakeWorkspaceResources::default();
        let service = service(catalog.clone(), resources.clone());

        let result = service
            .delete_workspace(&workspace.id)
            .await
            .expect("draft delete should succeed");

        assert!(result.workspace_catalog.workspaces.is_empty());
        assert!(catalog.stored_workspaces().is_empty());
        assert!(resources.calls().is_empty());
    }

    #[tokio::test]
    async fn delete_ready_workspace_cleans_resources_before_deleting_catalog_entry() {
        let mut workspace = ready_provisioning_workspace();
        workspace.lifecycle_state = WorkspaceLifecycleState::Ready;
        let catalog = FakeWorkspaceCatalog::with_workspaces([workspace.clone()]);
        let resources = FakeWorkspaceResources::default();
        resources.push_cleanup_result(Ok(workspace.clone()));
        let service = service(catalog.clone(), resources.clone());

        let result = service
            .delete_workspace(&workspace.id)
            .await
            .expect("ready delete should succeed");

        assert!(result.workspace_catalog.workspaces.is_empty());
        assert!(catalog.stored_workspaces().is_empty());
        assert_eq!(resources.calls(), vec![workspace.id]);
    }

    #[tokio::test]
    async fn delete_failed_workspace_with_cleanup_metadata_runs_cleanup() {
        let mut workspace = draft_workspace();
        workspace.lifecycle_state = WorkspaceLifecycleState::Failed;
        workspace.persistent_storage_volume_snapshot = Some(volume(
            crate::domain::workspace::ProviderResourceStatus::Ready,
        ));
        let catalog = FakeWorkspaceCatalog::with_workspaces([workspace.clone()]);
        let resources = FakeWorkspaceResources::default();
        resources.push_cleanup_result(Ok(workspace.clone()));
        let service = service(catalog, resources.clone());

        service
            .delete_workspace(&workspace.id)
            .await
            .expect("failed delete should succeed");

        assert_eq!(resources.calls(), vec![workspace.id]);
    }

    #[tokio::test]
    async fn delete_provisioning_workspace_is_rejected_without_cleanup_or_delete() {
        let mut workspace = draft_workspace();
        workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
        let catalog = FakeWorkspaceCatalog::with_workspaces([workspace.clone()]);
        let resources = FakeWorkspaceResources::default();
        let service = service(catalog.clone(), resources.clone());

        let error = service
            .delete_workspace(&workspace.id)
            .await
            .expect_err("provisioning delete should fail");

        assert_eq!(error, WorkspaceRemovalError::InvalidWorkspaceLifecycle);
        assert_eq!(catalog.stored_workspaces(), vec![workspace]);
        assert!(resources.calls().is_empty());
    }

    #[tokio::test]
    async fn delete_provisioning_workspace_returns_lifecycle_error_before_active_operation_conflict(
    ) {
        let mut workspace = draft_workspace();
        workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
        let catalog = FakeWorkspaceCatalog::with_workspaces([workspace.clone()]);
        let resources = FakeWorkspaceResources::default();
        let coordinator = WorkspaceProvisioningCoordinator::default();
        let _guard = coordinator
            .try_enter(&workspace.id)
            .expect("test should enter coordinator");
        let service = WorkspaceRemovalService::new(catalog.clone(), resources.clone(), coordinator);

        let error = service
            .delete_workspace(&workspace.id)
            .await
            .expect_err("provisioning delete should fail");

        assert_eq!(error, WorkspaceRemovalError::InvalidWorkspaceLifecycle);
        assert_eq!(catalog.stored_workspaces(), vec![workspace]);
        assert!(resources.calls().is_empty());
    }

    #[tokio::test]
    async fn delete_missing_workspace_returns_not_found() {
        let service = service(
            FakeWorkspaceCatalog::missing(),
            FakeWorkspaceResources::default(),
        );

        let error = service
            .delete_workspace("missing")
            .await
            .expect_err("missing delete should fail");

        assert_eq!(error, WorkspaceRemovalError::WorkspaceNotFound);
    }

    #[tokio::test]
    async fn cleanup_failure_preserves_workspace_for_retry() {
        let mut workspace = ready_provisioning_workspace();
        workspace.lifecycle_state = WorkspaceLifecycleState::Ready;
        let catalog = FakeWorkspaceCatalog::with_workspaces([workspace.clone()]);
        let resources = FakeWorkspaceResources::default();
        resources.push_cleanup_result(Err(WorkspaceResourceError::CleanupFailed));
        let service = service(catalog.clone(), resources);

        let error = service
            .delete_workspace(&workspace.id)
            .await
            .expect_err("cleanup failure should fail delete");

        assert_eq!(error, WorkspaceRemovalError::CleanupFailed);
        assert_eq!(catalog.stored_workspaces(), vec![workspace]);
    }

    #[tokio::test]
    async fn lifecycle_change_to_provisioning_before_delete_preserves_workspace() {
        let mut workspace = ready_provisioning_workspace();
        workspace.lifecycle_state = WorkspaceLifecycleState::Ready;
        let catalog = FakeWorkspaceCatalog::with_workspaces([workspace.clone()]);
        let resources = FakeWorkspaceResources::default();
        let catalog_for_callback = catalog.clone();
        resources.on_cleanup(move || {
            catalog_for_callback.set_lifecycle_state(WorkspaceLifecycleState::Provisioning);
        });
        resources.push_cleanup_result(Ok(workspace.clone()));
        let service = service(catalog.clone(), resources);

        let error = service
            .delete_workspace(&workspace.id)
            .await
            .expect_err("provisioning transition should prevent delete");

        assert_eq!(error, WorkspaceRemovalError::InvalidWorkspaceLifecycle);
        assert_eq!(
            catalog
                .stored_workspaces()
                .first()
                .expect("workspace should remain")
                .lifecycle_state,
            WorkspaceLifecycleState::Provisioning
        );
    }

    #[tokio::test]
    async fn delete_conflicts_with_active_workspace_operation() {
        let workspace = draft_workspace();
        let catalog = FakeWorkspaceCatalog::with_workspaces([workspace.clone()]);
        let resources = FakeWorkspaceResources::default();
        let coordinator = WorkspaceProvisioningCoordinator::default();
        let _guard = coordinator
            .try_enter(&workspace.id)
            .expect("test should enter coordinator");
        let service = WorkspaceRemovalService::new(catalog.clone(), resources, coordinator);

        let error = service
            .delete_workspace(&workspace.id)
            .await
            .expect_err("active operation should conflict");

        assert_eq!(error, WorkspaceRemovalError::ProviderOperationConflict);
        assert_eq!(catalog.stored_workspaces(), vec![workspace]);
    }

    #[tokio::test]
    async fn catalog_delete_failure_preserves_workspace() {
        let workspace = draft_workspace();
        let catalog = FakeWorkspaceCatalog::with_workspaces([workspace.clone()]);
        catalog.push_delete_error(WorkspaceSetupError::WorkspaceCatalogQueryFailed);
        let service = service(catalog.clone(), FakeWorkspaceResources::default());

        let error = service
            .delete_workspace(&workspace.id)
            .await
            .expect_err("catalog delete failure should fail");

        assert_eq!(error, WorkspaceRemovalError::WorkspaceCatalogQueryFailed);
        assert_eq!(catalog.stored_workspaces(), vec![workspace]);
    }

    fn service(
        catalog: FakeWorkspaceCatalog,
        resources: FakeWorkspaceResources,
    ) -> WorkspaceRemovalService<FakeWorkspaceCatalog, FakeWorkspaceResources> {
        WorkspaceRemovalService::new(
            catalog,
            resources,
            WorkspaceProvisioningCoordinator::default(),
        )
    }
}
