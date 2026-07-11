use time::OffsetDateTime;

use crate::application::{
    catalog::CatalogRef,
    events::{ApplicationEvent, ApplicationEventSink},
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
    events: &'a dyn ApplicationEventSink,
}

impl<'a> WorkspaceService<'a> {
    pub fn new(
        workspaces: &'a dyn WorkspaceRepository,
        lifecycle: &'a dyn LifecycleOperationRepository,
        workflows: &'a dyn WorkflowCatalog,
        events: &'a dyn ApplicationEventSink,
    ) -> Self {
        Self {
            workspaces,
            lifecycle,
            workflows,
            events,
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

        let workspace = self
            .workspaces
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
            })?;
        self.events
            .emit(ApplicationEvent::WorkspaceChanged(workspace.clone()));
        Ok(workspace)
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
            .ok_or(WorkspaceError::NotFound)?;
        self.events.emit(ApplicationEvent::WorkspaceDeleted {
            workspace_id: id.to_owned(),
        });
        Ok(())
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
        catalog::{
            CatalogRef, RunpodContractRequirements, RuntimeContractRequirements,
            WorkflowDefinition, WorkflowSummary,
        },
        events::{ApplicationEvent, ApplicationEventSink},
        lifecycle::{
            ports::{LifecycleOperationRepository, LifecycleOperationRepositoryError},
            LifecycleOperation, LifecycleOperationKind,
        },
        runtimes::{
            runpod::{RunpodProgress, RunpodProvisionStep},
            RuntimeKind, RuntimeProgress,
        },
        workspace::{
            ports::{
                WorkflowCatalog, WorkflowCatalogError, WorkspaceRepository,
                WorkspaceRepositoryError,
            },
            Workspace,
        },
    };

    #[derive(Default)]
    struct RecordingApplicationEventSink(Mutex<Vec<ApplicationEvent>>);

    impl RecordingApplicationEventSink {
        fn events(&self) -> Vec<ApplicationEvent> {
            self.0.lock().unwrap().clone()
        }
    }

    impl ApplicationEventSink for RecordingApplicationEventSink {
        fn emit(&self, event: ApplicationEvent) {
            self.0.lock().unwrap().push(event);
        }
    }

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
        workflow: Option<WorkflowDefinition>,
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
            Ok(self.workflow.clone().filter(|workflow| {
                workflow.summary.id == id && workflow.summary.revision == revision
            }))
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
        events: RecordingApplicationEventSink,
    }

    impl Fakes {
        fn with_missing_workflow() -> Self {
            Self::new(Vec::new(), Vec::new(), None)
        }

        fn with_workflow() -> Self {
            Self::new(Vec::new(), Vec::new(), Some(workflow()))
        }

        fn with_unprovisioned_workspace() -> Self {
            Self::new(
                vec![Workspace {
                    id: "workspace-1".into(),
                    workflow: CatalogRef::new("workflow", "1.0.0"),
                    created_at: OffsetDateTime::UNIX_EPOCH,
                    attached_runtime: None,
                }],
                Vec::new(),
                None,
            )
        }

        fn with_workspace(workspace: Workspace) -> Self {
            Self::new(vec![workspace], Vec::new(), None)
        }

        fn with_unprovisioned_workspace_and_running_operation() -> Self {
            Self::new(
                vec![Workspace {
                    id: "workspace-1".into(),
                    workflow: CatalogRef::new("workflow", "1.0.0"),
                    created_at: OffsetDateTime::UNIX_EPOCH,
                    attached_runtime: None,
                }],
                vec![LifecycleOperation::running(
                    Uuid::from_u128(1),
                    "workspace-1",
                    Uuid::from_u128(2),
                    LifecycleOperationKind::Provision,
                    RuntimeProgress::Runpod(RunpodProgress::Provision(
                        RunpodProvisionStep::CreateNetworkVolume,
                    )),
                    OffsetDateTime::UNIX_EPOCH,
                )],
                None,
            )
        }

        fn new(
            workspaces: Vec<Workspace>,
            operations: Vec<LifecycleOperation>,
            workflow: Option<WorkflowDefinition>,
        ) -> Self {
            Self {
                workspaces: FakeWorkspaceRepository::new(workspaces),
                workflows: FakeWorkflowCatalog {
                    gets: Mutex::new(Vec::new()),
                    workflow,
                },
                lifecycle: FakeLifecycleOperationRepository {
                    operations: Mutex::new(operations),
                    running_checks: Mutex::new(Vec::new()),
                },
                events: RecordingApplicationEventSink::default(),
            }
        }

        fn service(&self) -> WorkspaceService<'_> {
            WorkspaceService::new(
                &self.workspaces,
                &self.lifecycle,
                &self.workflows,
                &self.events,
            )
        }
    }

    fn workflow() -> WorkflowDefinition {
        WorkflowDefinition {
            summary: WorkflowSummary {
                id: "workflow".into(),
                revision: "1.0.0".into(),
                name: "Workflow".into(),
                description: "Workflow description".into(),
                required_volume_size_gb: 1,
                requires_hugging_face_api_key: false,
            },
            runtime_preset_ref: CatalogRef::new("runpod-preset", "1.0.0"),
            contract_requirements: vec![RuntimeContractRequirements::Runpod(
                RunpodContractRequirements {
                    provisioner_contract_ref: CatalogRef::new("provisioner", "1.0.0"),
                    endpoint_contract_ref: CatalogRef::new("endpoint", "1.0.0"),
                },
            )],
            model_assets: serde_json::json!([]),
            execution_contract: serde_json::json!({}),
            workflow_graph: serde_json::json!({}),
        }
    }

    #[tokio::test]
    async fn create_emits_the_committed_workspace() {
        let fakes = Fakes::with_workflow();

        let workspace = fakes
            .service()
            .create("workspace-1", CatalogRef::new("workflow", "1.0.0"))
            .await
            .unwrap();

        assert_eq!(
            fakes.events.events(),
            vec![ApplicationEvent::WorkspaceChanged(workspace)],
        );
    }

    #[tokio::test]
    async fn delete_emits_only_after_the_workspace_is_removed() {
        let fakes = Fakes::with_unprovisioned_workspace();

        fakes.service().delete("workspace-1").await.unwrap();

        assert!(!fakes.workspaces.contains("workspace-1"));
        assert_eq!(
            fakes.events.events(),
            vec![ApplicationEvent::WorkspaceDeleted {
                workspace_id: "workspace-1".into(),
            }],
        );
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
        assert!(fakes.events.events().is_empty());
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
        assert!(fakes.events.events().is_empty());
    }

    #[tokio::test]
    async fn delete_rejects_a_running_operation_and_preserves_history() {
        let fakes = Fakes::with_unprovisioned_workspace_and_running_operation();

        assert_eq!(
            fakes.service().delete("workspace-1").await,
            Err(WorkspaceError::OperationRunning)
        );
        assert!(fakes.workspaces.contains("workspace-1"));
        assert!(fakes.events.events().is_empty());
    }
}
