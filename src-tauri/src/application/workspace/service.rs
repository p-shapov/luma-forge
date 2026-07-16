use std::sync::Arc;

use time::OffsetDateTime;
use uuid::Uuid;

use crate::application::{
    events::{ApplicationEvent, ApplicationEventSink},
    runtimes::{CatalogRef, WorkflowSummary},
    workspace::{
        ports::{WorkflowCatalog, WorkspaceRepository, WorkspaceRepositoryError},
        Workspace, WorkspaceError,
    },
};

#[derive(Clone)]
pub struct WorkspaceService {
    workspaces: Arc<dyn WorkspaceRepository>,
    workflows: Arc<dyn WorkflowCatalog>,
    events: Arc<dyn ApplicationEventSink>,
}

impl WorkspaceService {
    pub fn new(
        workspaces: Arc<dyn WorkspaceRepository>,
        workflows: Arc<dyn WorkflowCatalog>,
        events: Arc<dyn ApplicationEventSink>,
    ) -> Self {
        Self {
            workspaces,
            workflows,
            events,
        }
    }

    #[crate::diagnostics::diagnostic(show_output, show_error)]
    pub async fn create(
        &self,
        #[diagnostic(show)] workflow: CatalogRef,
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
                id: Uuid::new_v4().to_string(),
                workflow,
                created_at: OffsetDateTime::now_utc(),
                runtime: None,
            })
            .await
            .map_err(|error| match error {
                WorkspaceRepositoryError::AlreadyExists => WorkspaceError::AlreadyExists,
                WorkspaceRepositoryError::RuntimeAttached
                | WorkspaceRepositoryError::OperationRunning
                | WorkspaceRepositoryError::Unavailable
                | WorkspaceRepositoryError::CorruptData => WorkspaceError::PersistenceUnavailable,
            })?;
        self.events
            .emit(ApplicationEvent::WorkspaceChanged(workspace.clone()));
        Ok(workspace)
    }

    #[crate::diagnostics::diagnostic(show_error)]
    pub async fn delete(&self, #[diagnostic(show)] id: &str) -> Result<(), WorkspaceError> {
        self.workspaces
            .delete(id)
            .await
            .map_err(|error| match error {
                WorkspaceRepositoryError::RuntimeAttached => WorkspaceError::RuntimeAttached,
                WorkspaceRepositoryError::OperationRunning => WorkspaceError::OperationRunning,
                WorkspaceRepositoryError::AlreadyExists
                | WorkspaceRepositoryError::Unavailable
                | WorkspaceRepositoryError::CorruptData => WorkspaceError::PersistenceUnavailable,
            })?
            .then_some(())
            .ok_or(WorkspaceError::NotFound)?;
        self.events.emit(ApplicationEvent::WorkspaceDeleted {
            workspace_id: id.to_owned(),
        });
        Ok(())
    }

    #[crate::diagnostics::diagnostic(show_output, show_error)]
    pub async fn get(&self, #[diagnostic(show)] id: &str) -> Result<Workspace, WorkspaceError> {
        self.workspaces
            .get(id)
            .await
            .map_err(|_| WorkspaceError::PersistenceUnavailable)?
            .ok_or(WorkspaceError::NotFound)
    }

    #[crate::diagnostics::diagnostic(show_output, show_error)]
    pub async fn list_workflows(
        &self,
        #[diagnostic(show)] offset: u64,
        #[diagnostic(show)] limit: u64,
    ) -> Result<(Vec<WorkflowSummary>, u64), WorkspaceError> {
        let summaries = self
            .workflows
            .list_summaries()
            .await
            .map_err(|_| WorkspaceError::CatalogUnavailable)?;
        let total = summaries.len() as u64;
        let offset = usize::try_from(offset).unwrap_or(usize::MAX);
        let limit = usize::try_from(limit).unwrap_or(usize::MAX);
        Ok((
            summaries.into_iter().skip(offset).take(limit).collect(),
            total,
        ))
    }

    #[crate::diagnostics::diagnostic(show_output, show_error)]
    pub async fn list(
        &self,
        #[diagnostic(show)] offset: u64,
        #[diagnostic(show)] limit: u64,
    ) -> Result<(Vec<Workspace>, u64), WorkspaceError> {
        self.workspaces
            .page(offset, limit)
            .await
            .map_err(|_| WorkspaceError::PersistenceUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use time::OffsetDateTime;
    use tokio::sync::Barrier;

    use super::{WorkspaceError, WorkspaceService};
    use crate::{
        adapters::sqlite::{SqliteRuntimeTransitionRepository, SqliteWorkspaceRepository},
        application::{
            events::{ApplicationEvent, ApplicationEventSink},
            runtimes::{
                runpod::{
                    test_support::{provision_command, ProvisionFakes},
                    RunpodContractRequirements, RunpodRuntime, RunpodRuntimeConfig,
                },
                CatalogRef, Runtime, RuntimeContractRequirements, RuntimeProvider, RuntimeState,
                WorkflowDefinition, WorkflowSummary,
            },
            workspace::{
                ports::{
                    WorkflowCatalog, WorkflowCatalogError, WorkspaceRepository,
                    WorkspaceRepositoryError,
                },
                Workspace,
            },
        },
        infra::sqlite::database::SqliteInfraDatabase,
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
        has_running: bool,
    }

    impl FakeWorkspaceRepository {
        fn new(workspaces: Vec<Workspace>, has_running: bool) -> Self {
            Self {
                workspaces: Mutex::new(workspaces),
                creates: Mutex::new(Vec::new()),
                has_running,
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

        async fn page(
            &self,
            offset: u64,
            limit: u64,
        ) -> Result<(Vec<Workspace>, u64), WorkspaceRepositoryError> {
            let workspaces = self.workspaces.lock().unwrap();
            let total = workspaces.len() as u64;
            Ok((
                workspaces
                    .iter()
                    .skip(usize::try_from(offset).unwrap_or(usize::MAX))
                    .take(usize::try_from(limit).unwrap_or(usize::MAX))
                    .cloned()
                    .collect(),
                total,
            ))
        }

        async fn delete(&self, id: &str) -> Result<bool, WorkspaceRepositoryError> {
            let mut workspaces = self.workspaces.lock().unwrap();
            let Some(index) = workspaces.iter().position(|workspace| workspace.id == id) else {
                return Ok(false);
            };
            if workspaces[index].runtime.is_some() {
                return Err(WorkspaceRepositoryError::RuntimeAttached);
            }
            if self.has_running {
                return Err(WorkspaceRepositoryError::OperationRunning);
            }
            workspaces.remove(index);
            Ok(true)
        }
    }

    struct PausedDeleteWorkspaceRepository {
        inner: Arc<SqliteWorkspaceRepository>,
        delete_entered: Arc<Barrier>,
        resume_delete: Arc<Barrier>,
    }

    #[async_trait::async_trait]
    impl WorkspaceRepository for PausedDeleteWorkspaceRepository {
        async fn create(
            &self,
            workspace: Workspace,
        ) -> Result<Workspace, WorkspaceRepositoryError> {
            self.inner.create(workspace).await
        }

        async fn get(&self, id: &str) -> Result<Option<Workspace>, WorkspaceRepositoryError> {
            self.inner.get(id).await
        }

        async fn page(
            &self,
            offset: u64,
            limit: u64,
        ) -> Result<(Vec<Workspace>, u64), WorkspaceRepositoryError> {
            self.inner.page(offset, limit).await
        }

        async fn delete(&self, id: &str) -> Result<bool, WorkspaceRepositoryError> {
            self.delete_entered.wait().await;
            self.resume_delete.wait().await;
            self.inner.delete(id).await
        }
    }

    struct FakeWorkflowCatalog {
        gets: Mutex<Vec<CatalogRef>>,
        workflows: Vec<WorkflowDefinition>,
    }

    #[async_trait::async_trait]
    impl WorkflowCatalog for FakeWorkflowCatalog {
        async fn list_summaries(&self) -> Result<Vec<WorkflowSummary>, WorkflowCatalogError> {
            Ok(self
                .workflows
                .iter()
                .map(|workflow| workflow.summary.clone())
                .collect())
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
            Ok(self
                .workflows
                .iter()
                .find(|workflow| workflow.summary.id == id && workflow.summary.revision == revision)
                .cloned())
        }
    }

    struct Fakes {
        workspaces: Arc<FakeWorkspaceRepository>,
        workflows: Arc<FakeWorkflowCatalog>,
        events: Arc<RecordingApplicationEventSink>,
    }

    impl Fakes {
        fn with_missing_workflow() -> Self {
            Self::new(Vec::new(), false, Vec::new())
        }

        fn with_workflow() -> Self {
            Self::new(Vec::new(), false, vec![workflow()])
        }

        fn with_workflows(workflows: Vec<WorkflowDefinition>) -> Self {
            Self::new(Vec::new(), false, workflows)
        }

        fn with_unprovisioned_workspace() -> Self {
            Self::new(
                vec![Workspace {
                    id: "workspace-1".into(),
                    workflow: CatalogRef::new("workflow", "1.0.0"),
                    created_at: OffsetDateTime::UNIX_EPOCH,
                    runtime: None,
                }],
                false,
                Vec::new(),
            )
        }

        fn with_workspace(workspace: Workspace) -> Self {
            Self::new(vec![workspace], false, Vec::new())
        }

        fn with_unprovisioned_workspace_and_running_operation() -> Self {
            Self::new(
                vec![Workspace {
                    id: "workspace-1".into(),
                    workflow: CatalogRef::new("workflow", "1.0.0"),
                    created_at: OffsetDateTime::UNIX_EPOCH,
                    runtime: None,
                }],
                true,
                Vec::new(),
            )
        }

        fn new(
            workspaces: Vec<Workspace>,
            has_running: bool,
            workflows: Vec<WorkflowDefinition>,
        ) -> Self {
            Self {
                workspaces: Arc::new(FakeWorkspaceRepository::new(workspaces, has_running)),
                workflows: Arc::new(FakeWorkflowCatalog {
                    gets: Mutex::new(Vec::new()),
                    workflows,
                }),
                events: Arc::new(RecordingApplicationEventSink::default()),
            }
        }

        fn service(&self) -> WorkspaceService {
            WorkspaceService::new(
                self.workspaces.clone(),
                self.workflows.clone(),
                self.events.clone(),
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

    fn three_workflows() -> Vec<WorkflowDefinition> {
        (1..=3)
            .map(|index| {
                let mut workflow = workflow();
                workflow.summary.id = format!("workflow-{index}");
                workflow
            })
            .collect()
    }

    #[tokio::test]
    async fn create_generates_a_uuid() {
        let fakes = Fakes::with_workflow();
        let workspace = fakes
            .service()
            .create(CatalogRef::new("workflow", "1.0.0"))
            .await
            .unwrap();
        assert!(uuid::Uuid::parse_str(&workspace.id).is_ok());
    }

    #[tokio::test]
    async fn workflow_page_returns_total_before_paging() {
        let fakes = Fakes::with_workflows(three_workflows());
        let (items, total) = fakes.service().list_workflows(1, 1).await.unwrap();
        assert_eq!(total, 3);
        assert_eq!(items.len(), 1);
    }

    #[tokio::test]
    async fn create_emits_the_committed_workspace() {
        let fakes = Fakes::with_workflow();

        let workspace = fakes
            .service()
            .create(CatalogRef::new("workflow", "1.0.0"))
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

        let result = service.create(CatalogRef::new("missing", "1.0.0")).await;

        assert_eq!(result, Err(WorkspaceError::WorkflowNotFound));
        assert!(fakes.workspaces.created().is_empty());
        assert!(fakes.events.events().is_empty());
    }

    #[tokio::test]
    async fn delete_rejects_a_runtime() {
        let fakes = Fakes::with_workspace(Workspace {
            id: "workspace-1".into(),
            workflow: CatalogRef::new("workflow", "1.0.0"),
            created_at: OffsetDateTime::UNIX_EPOCH,
            runtime: Some(Runtime {
                state: RuntimeState::Ready,
                provider: RuntimeProvider::Runpod(RunpodRuntime::new_provisioning(
                    uuid::Uuid::from_u128(1),
                    RunpodRuntimeConfig {
                        datacenter_id: "dc-1".into(),
                        gpu_id: "gpu-1".into(),
                        volume_size_gb: 19,
                    },
                )),
            }),
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

    #[tokio::test]
    async fn provision_admission_wins_over_an_in_flight_delete_while_provider_work_starts() {
        let path = std::env::temp_dir().join(format!("luma-forge-{}.sqlite", uuid::Uuid::new_v4()));
        let database = SqliteInfraDatabase::connect(path).await.unwrap();
        let connection = database.connection().clone();
        let inner = Arc::new(SqliteWorkspaceRepository::new(connection.clone()));
        inner
            .create(Workspace {
                id: "workspace-1".into(),
                workflow: CatalogRef::new("workflow-1", "1"),
                created_at: OffsetDateTime::UNIX_EPOCH,
                runtime: None,
            })
            .await
            .unwrap();
        let delete_entered = Arc::new(Barrier::new(2));
        let resume_delete = Arc::new(Barrier::new(2));
        let workspaces = Arc::new(PausedDeleteWorkspaceRepository {
            inner: inner.clone(),
            delete_entered: delete_entered.clone(),
            resume_delete: resume_delete.clone(),
        });
        let runtime_fakes = ProvisionFakes::ready();
        runtime_fakes.block_first_provider_call();
        let runtime_service = runtime_fakes.service_with_persistence(
            workspaces.clone(),
            Arc::new(SqliteRuntimeTransitionRepository::new(connection.clone())),
        );
        let workspace_service = WorkspaceService::new(
            workspaces,
            Arc::new(FakeWorkflowCatalog {
                gets: Mutex::new(Vec::new()),
                workflows: Vec::new(),
            }),
            Arc::new(RecordingApplicationEventSink::default()),
        );

        let delete = tokio::spawn(async move { workspace_service.delete("workspace-1").await });
        delete_entered.wait().await;
        runtime_service
            .start_provision(provision_command())
            .await
            .unwrap();
        runtime_fakes.wait_until_first_provider_call().await;
        resume_delete.wait().await;

        assert_eq!(delete.await.unwrap(), Err(WorkspaceError::RuntimeAttached));
        assert!(inner
            .get("workspace-1")
            .await
            .unwrap()
            .unwrap()
            .runtime
            .is_some());
        runtime_fakes.release_first_provider_call();
    }
}
