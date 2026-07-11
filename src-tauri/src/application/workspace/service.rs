use time::OffsetDateTime;

use crate::application::{
    catalog::CatalogRef,
    lifecycle::ports::LifecycleOperationRepository,
    workspace::{
        ports::{WorkflowCatalog, WorkspaceRepository, WorkspaceRepositoryError},
        Workspace, WorkspaceError,
    },
};

pub struct WorkspaceService<'a> {
    workspaces: &'a dyn WorkspaceRepository,
    lifecycle: &'a dyn LifecycleOperationRepository,
    workflows: &'a dyn WorkflowCatalog,
}

impl<'a> WorkspaceService<'a> {
    pub fn new(
        workspaces: &'a dyn WorkspaceRepository,
        lifecycle: &'a dyn LifecycleOperationRepository,
        workflows: &'a dyn WorkflowCatalog,
    ) -> Self {
        Self {
            workspaces,
            lifecycle,
            workflows,
        }
    }

    pub async fn create(
        &self,
        id: &str,
        workflow: CatalogRef,
    ) -> Result<Workspace, WorkspaceError> {
        if self
            .workflows
            .get(&workflow.id, &workflow.revision)
            .await
            .map_err(|_| WorkspaceError::CatalogUnavailable)?
            .is_none()
        {
            return Err(WorkspaceError::WorkflowNotFound);
        }

        self.workspaces
            .create(Workspace {
                id: id.to_owned(),
                workflow,
                created_at: OffsetDateTime::now_utc(),
                attached_runtime: None,
            })
            .await
            .map_err(|error| match error {
                WorkspaceRepositoryError::AlreadyExists => WorkspaceError::AlreadyExists,
                WorkspaceRepositoryError::Unavailable | WorkspaceRepositoryError::CorruptData => {
                    WorkspaceError::PersistenceUnavailable
                }
            })
    }

    pub async fn delete(&self, id: &str) -> Result<(), WorkspaceError> {
        let workspace = self.get(id).await?;
        if workspace.attached_runtime.is_some() {
            return Err(WorkspaceError::RuntimeAttached);
        }
        if self
            .lifecycle
            .has_running(id)
            .await
            .map_err(|_| WorkspaceError::PersistenceUnavailable)?
        {
            return Err(WorkspaceError::OperationRunning);
        }

        self.workspaces
            .delete(id)
            .await
            .map_err(|_| WorkspaceError::PersistenceUnavailable)?
            .then_some(())
            .ok_or(WorkspaceError::NotFound)
    }

    pub async fn get(&self, id: &str) -> Result<Workspace, WorkspaceError> {
        self.workspaces
            .get(id)
            .await
            .map_err(|_| WorkspaceError::PersistenceUnavailable)?
            .ok_or(WorkspaceError::NotFound)
    }

    pub async fn list(&self) -> Result<Vec<Workspace>, WorkspaceError> {
        self.workspaces
            .list()
            .await
            .map_err(|_| WorkspaceError::PersistenceUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use time::OffsetDateTime;
    use uuid::Uuid;

    use super::{WorkspaceError, WorkspaceService};
    use crate::application::{
        catalog::{CatalogRef, WorkflowDefinition, WorkflowSummary},
        lifecycle::{
            ports::{LifecycleOperationRepository, LifecycleOperationRepositoryError},
            progress::runpod::RunpodProvisionStep,
            LifecycleOperation,
        },
        workspace::{
            ports::{
                WorkflowCatalog, WorkflowCatalogError, WorkspaceRepository,
                WorkspaceRepositoryError,
            },
            RuntimeKind, Workspace,
        },
    };

    struct FakeWorkspaceRepository {
        workspaces: Mutex<Vec<Workspace>>,
        creates: Mutex<Vec<Workspace>>,
    }

    impl FakeWorkspaceRepository {
        fn new(workspaces: Vec<Workspace>) -> Self {
            Self {
                workspaces: Mutex::new(workspaces),
                creates: Mutex::new(Vec::new()),
            }
        }

        fn created(&self) -> Vec<Workspace> {
            self.creates.lock().unwrap().clone()
        }

        fn contains(&self, id: &str) -> bool {
            self.workspaces
                .lock()
                .unwrap()
                .iter()
                .any(|workspace| workspace.id == id)
        }
    }

    #[async_trait::async_trait]
    impl WorkspaceRepository for FakeWorkspaceRepository {
        async fn create(
            &self,
            workspace: Workspace,
        ) -> Result<Workspace, WorkspaceRepositoryError> {
            self.creates.lock().unwrap().push(workspace.clone());
            self.workspaces.lock().unwrap().push(workspace.clone());
            Ok(workspace)
        }

        async fn get(&self, id: &str) -> Result<Option<Workspace>, WorkspaceRepositoryError> {
            Ok(self
                .workspaces
                .lock()
                .unwrap()
                .iter()
                .find(|workspace| workspace.id == id)
                .cloned())
        }

        async fn list(&self) -> Result<Vec<Workspace>, WorkspaceRepositoryError> {
            Ok(self.workspaces.lock().unwrap().clone())
        }

        async fn delete(&self, id: &str) -> Result<bool, WorkspaceRepositoryError> {
            let mut workspaces = self.workspaces.lock().unwrap();
            let original_len = workspaces.len();
            workspaces.retain(|workspace| workspace.id != id);
            Ok(workspaces.len() != original_len)
        }
    }

    struct FakeWorkflowCatalog {
        gets: Mutex<Vec<CatalogRef>>,
    }

    #[async_trait::async_trait]
    impl WorkflowCatalog for FakeWorkflowCatalog {
        async fn list_summaries(&self) -> Result<Vec<WorkflowSummary>, WorkflowCatalogError> {
            Ok(Vec::new())
        }

        async fn get(
            &self,
            id: &str,
            revision: &str,
        ) -> Result<Option<WorkflowDefinition>, WorkflowCatalogError> {
            self.gets
                .lock()
                .unwrap()
                .push(CatalogRef::new(id, revision));
            Ok(None)
        }
    }

    struct FakeLifecycleOperationRepository {
        operations: Mutex<Vec<LifecycleOperation>>,
        running_checks: Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl LifecycleOperationRepository for FakeLifecycleOperationRepository {
        async fn recent(
            &self,
            limit: u64,
        ) -> Result<Vec<LifecycleOperation>, LifecycleOperationRepositoryError> {
            Ok(self
                .operations
                .lock()
                .unwrap()
                .iter()
                .take(limit as usize)
                .cloned()
                .collect())
        }

        async fn recent_for_workspace(
            &self,
            workspace_id: &str,
            limit: u64,
        ) -> Result<Vec<LifecycleOperation>, LifecycleOperationRepositoryError> {
            Ok(self
                .operations
                .lock()
                .unwrap()
                .iter()
                .filter(|operation| operation.workspace_id == workspace_id)
                .take(limit as usize)
                .cloned()
                .collect())
        }

        async fn running(
            &self,
        ) -> Result<Vec<LifecycleOperation>, LifecycleOperationRepositoryError> {
            Ok(self.operations.lock().unwrap().clone())
        }

        async fn has_running(
            &self,
            workspace_id: &str,
        ) -> Result<bool, LifecycleOperationRepositoryError> {
            self.running_checks
                .lock()
                .unwrap()
                .push(workspace_id.to_owned());
            Ok(self
                .operations
                .lock()
                .unwrap()
                .iter()
                .any(|operation| operation.workspace_id == workspace_id))
        }
    }

    struct Fakes {
        workspaces: FakeWorkspaceRepository,
        workflows: FakeWorkflowCatalog,
        lifecycle: FakeLifecycleOperationRepository,
    }

    impl Fakes {
        fn with_missing_workflow() -> Self {
            Self::new(Vec::new(), Vec::new())
        }

        fn with_workspace(workspace: Workspace) -> Self {
            Self::new(vec![workspace], Vec::new())
        }

        fn with_unprovisioned_workspace_and_running_operation() -> Self {
            Self::new(
                vec![Workspace {
                    id: "workspace-1".into(),
                    workflow: CatalogRef::new("workflow", "1.0.0"),
                    created_at: OffsetDateTime::UNIX_EPOCH,
                    attached_runtime: None,
                }],
                vec![LifecycleOperation::runpod_provision(
                    Uuid::from_u128(1),
                    "workspace-1",
                    Uuid::from_u128(2),
                    RunpodProvisionStep::CreateNetworkVolume,
                    OffsetDateTime::UNIX_EPOCH,
                )],
            )
        }

        fn new(workspaces: Vec<Workspace>, operations: Vec<LifecycleOperation>) -> Self {
            Self {
                workspaces: FakeWorkspaceRepository::new(workspaces),
                workflows: FakeWorkflowCatalog {
                    gets: Mutex::new(Vec::new()),
                },
                lifecycle: FakeLifecycleOperationRepository {
                    operations: Mutex::new(operations),
                    running_checks: Mutex::new(Vec::new()),
                },
            }
        }

        fn service(&self) -> WorkspaceService<'_> {
            WorkspaceService::new(&self.workspaces, &self.lifecycle, &self.workflows)
        }
    }

    #[tokio::test]
    async fn create_rejects_an_unknown_workflow_without_writing() {
        let fakes = Fakes::with_missing_workflow();
        let service = fakes.service();

        let result = service
            .create("workspace-1", CatalogRef::new("missing", "1.0.0"))
            .await;

        assert_eq!(result, Err(WorkspaceError::WorkflowNotFound));
        assert!(fakes.workspaces.created().is_empty());
    }

    #[tokio::test]
    async fn delete_rejects_an_attached_runtime() {
        let fakes = Fakes::with_workspace(Workspace {
            id: "workspace-1".into(),
            workflow: CatalogRef::new("workflow", "1.0.0"),
            created_at: OffsetDateTime::UNIX_EPOCH,
            attached_runtime: Some(RuntimeKind::Runpod),
        });

        assert_eq!(
            fakes.service().delete("workspace-1").await,
            Err(WorkspaceError::RuntimeAttached)
        );
    }

    #[tokio::test]
    async fn delete_rejects_a_running_operation_and_preserves_history() {
        let fakes = Fakes::with_unprovisioned_workspace_and_running_operation();

        assert_eq!(
            fakes.service().delete("workspace-1").await,
            Err(WorkspaceError::OperationRunning)
        );
        assert!(fakes.workspaces.contains("workspace-1"));
    }
}
