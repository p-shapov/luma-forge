use std::sync::Arc;

use crate::application::{
    events::{ApplicationEvent, ApplicationEventSink},
    runtimes::{
        ports::{RuntimeTransitionRepository, RuntimeTransitionRepositoryError},
        RuntimeModel, RuntimeOperation,
    },
    workspace::ports::WorkspaceRepository,
};

pub struct RuntimeTransitionContext<R, P>
where
    R: RuntimeModel,
    P: RuntimeTransitionRepository<R> + ?Sized,
{
    transitions: Arc<P>,
    workspaces: Arc<dyn WorkspaceRepository>,
    events: Arc<dyn ApplicationEventSink>,
    coordinator: Arc<tokio::sync::Mutex<()>>,
    runtime: std::marker::PhantomData<R>,
}

impl<R, P> Clone for RuntimeTransitionContext<R, P>
where
    R: RuntimeModel,
    P: RuntimeTransitionRepository<R> + ?Sized,
{
    fn clone(&self) -> Self {
        Self {
            transitions: self.transitions.clone(),
            workspaces: self.workspaces.clone(),
            events: self.events.clone(),
            coordinator: self.coordinator.clone(),
            runtime: std::marker::PhantomData,
        }
    }
}

impl<R, P> RuntimeTransitionContext<R, P>
where
    R: RuntimeModel,
    P: RuntimeTransitionRepository<R> + ?Sized,
{
    pub fn new(
        transitions: Arc<P>,
        workspaces: Arc<dyn WorkspaceRepository>,
        events: Arc<dyn ApplicationEventSink>,
    ) -> Self {
        Self {
            transitions,
            workspaces,
            events,
            coordinator: Arc::new(tokio::sync::Mutex::new(())),
            runtime: std::marker::PhantomData,
        }
    }

    pub fn transitions(&self) -> &P {
        self.transitions.as_ref()
    }

    pub async fn save_changed(
        &self,
        runtime: &R,
        operation: &RuntimeOperation,
    ) -> Result<(), RuntimeTransitionRepositoryError> {
        let _guard = self.coordinator.lock().await;
        self.transitions.save_transition(runtime, operation).await?;
        self.events.emit(ApplicationEvent::RuntimeChanged(
            runtime.clone().into_runtime(),
        ));
        self.events
            .emit(ApplicationEvent::RuntimeOperationChanged(operation.clone()));
        Ok(())
    }

    pub async fn save_attached(
        &self,
        runtime: &R,
        operation: &RuntimeOperation,
    ) -> Result<(), RuntimeTransitionRepositoryError> {
        let _guard = self.coordinator.lock().await;
        self.transitions.save_transition(runtime, operation).await?;
        self.emit_workspace_projection(runtime.workspace_id()).await;
        self.events.emit(ApplicationEvent::RuntimeChanged(
            runtime.clone().into_runtime(),
        ));
        self.events
            .emit(ApplicationEvent::RuntimeOperationChanged(operation.clone()));
        Ok(())
    }

    pub async fn save_deleted(
        &self,
        runtime: &R,
        operation: &RuntimeOperation,
    ) -> Result<(), RuntimeTransitionRepositoryError> {
        let _guard = self.coordinator.lock().await;
        self.transitions.save_transition(runtime, operation).await?;
        self.emit_workspace_projection(runtime.workspace_id()).await;
        self.events.emit(ApplicationEvent::RuntimeDeleted {
            workspace_id: runtime.workspace_id().to_owned(),
            kind: runtime.kind(),
        });
        self.events
            .emit(ApplicationEvent::RuntimeOperationChanged(operation.clone()));
        Ok(())
    }

    async fn emit_workspace_projection(&self, workspace_id: &str) {
        if let Ok(Some(workspace)) = self.workspaces.get(workspace_id).await {
            self.events
                .emit(ApplicationEvent::WorkspaceChanged(workspace));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    };

    use time::OffsetDateTime;
    use tokio::sync::oneshot;
    use uuid::Uuid;

    use crate::application::{
        events::{ApplicationEvent, ApplicationEventSink},
        runtimes::{
            ports::{RuntimeTransitionRepository, RuntimeTransitionRepositoryError},
            runpod::{
                RunpodProgress, RunpodProvisionStep, RunpodRuntime, RunpodRuntimeConfig,
                RunpodRuntimeResources, RunpodRuntimeState,
            },
            CatalogRef, Runtime, RuntimeKind, RuntimeOperation, RuntimeOperationKind,
            RuntimeProgress,
        },
        workspace::{
            ports::{WorkspaceRepository, WorkspaceRepositoryError},
            Workspace,
        },
    };

    use super::RuntimeTransitionContext;

    #[tokio::test]
    async fn attached_transition_commits_before_ordered_events() {
        let fakes = Fakes::attached();

        fakes
            .context()
            .save_attached(&fakes.runtime, &fakes.operation)
            .await
            .unwrap();

        assert_eq!(
            fakes.events.events(),
            vec![
                ApplicationEvent::WorkspaceChanged(fakes.workspace.clone()),
                ApplicationEvent::RuntimeChanged(Runtime::Runpod(fakes.runtime.clone())),
                ApplicationEvent::RuntimeOperationChanged(fakes.operation.clone()),
            ]
        );
        assert!(fakes.events.all_emitted_after_commit());
    }

    #[tokio::test]
    async fn changed_transition_emits_only_runtime_then_runtime_operation() {
        let fakes = Fakes::attached();

        fakes
            .context()
            .save_changed(&fakes.runtime, &fakes.operation)
            .await
            .unwrap();

        assert_eq!(
            fakes.events.events(),
            vec![
                ApplicationEvent::RuntimeChanged(Runtime::Runpod(fakes.runtime.clone())),
                ApplicationEvent::RuntimeOperationChanged(fakes.operation.clone()),
            ]
        );
    }

    #[tokio::test]
    async fn deleted_transition_emits_detached_workspace_before_deletion_and_runtime_operation() {
        let fakes = Fakes::detached();

        fakes
            .context()
            .save_deleted(&fakes.runtime, &fakes.operation)
            .await
            .unwrap();

        assert_eq!(
            fakes.events.events(),
            vec![
                ApplicationEvent::WorkspaceChanged(fakes.workspace.clone()),
                ApplicationEvent::RuntimeDeleted {
                    workspace_id: "workspace-1".into(),
                    kind: RuntimeKind::Runpod,
                },
                ApplicationEvent::RuntimeOperationChanged(fakes.operation.clone()),
            ]
        );
    }

    #[tokio::test]
    async fn failed_commit_emits_nothing() {
        let fakes = Fakes::failing_transition();

        assert_eq!(
            fakes
                .context()
                .save_changed(&fakes.runtime, &fakes.operation)
                .await,
            Err(RuntimeTransitionRepositoryError::Unavailable),
        );
        assert!(fakes.events.events().is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cleanup_events_complete_before_reprovision_events() {
        let fakes = Fakes::detached();
        let (projection_entered_tx, projection_entered_rx) = oneshot::channel();
        let (release_projection_tx, release_projection_rx) = oneshot::channel();
        let workspaces = Arc::new(BlockingWorkspaceRepository {
            workspace: fakes.workspace.clone(),
            gets: AtomicUsize::new(0),
            projection_entered: Mutex::new(Some(projection_entered_tx)),
            release_projection: Mutex::new(Some(release_projection_rx)),
        });
        let context = RuntimeTransitionContext::new(
            fakes.transitions.clone(),
            workspaces,
            fakes.events.clone(),
        );

        let cleanup_context = context.clone();
        let cleanup_runtime = fakes.runtime.clone();
        let cleanup_operation = fakes.operation.clone();
        let cleanup = tokio::spawn(async move {
            cleanup_context
                .save_deleted(&cleanup_runtime, &cleanup_operation)
                .await
        });
        projection_entered_rx.await.unwrap();

        let (reprovision_started_tx, reprovision_started_rx) = oneshot::channel();
        let reprovision_runtime = fakes.runtime.clone();
        let reprovision_operation = fakes.operation.clone();
        let reprovision = tokio::spawn(async move {
            reprovision_started_tx.send(()).unwrap();
            context
                .save_attached(&reprovision_runtime, &reprovision_operation)
                .await
        });
        reprovision_started_rx.await.unwrap();

        assert!(fakes.events.events().is_empty());
        release_projection_tx.send(()).unwrap();
        cleanup.await.unwrap().unwrap();
        reprovision.await.unwrap().unwrap();

        assert_eq!(
            fakes.events.events(),
            vec![
                ApplicationEvent::WorkspaceChanged(fakes.workspace.clone()),
                ApplicationEvent::RuntimeDeleted {
                    workspace_id: "workspace-1".into(),
                    kind: RuntimeKind::Runpod,
                },
                ApplicationEvent::RuntimeOperationChanged(fakes.operation.clone()),
                ApplicationEvent::WorkspaceChanged(fakes.workspace.clone()),
                ApplicationEvent::RuntimeChanged(Runtime::Runpod(fakes.runtime.clone())),
                ApplicationEvent::RuntimeOperationChanged(fakes.operation.clone()),
            ]
        );
    }

    struct Fakes {
        transitions: Arc<FakeTransitionRepository>,
        workspaces: Arc<FakeWorkspaceRepository>,
        events: Arc<RecordingEventSink>,
        runtime: RunpodRuntime,
        operation: RuntimeOperation,
        workspace: Workspace,
    }

    impl Fakes {
        fn attached() -> Self {
            Self::new(Some(RuntimeKind::Runpod), false)
        }

        fn detached() -> Self {
            Self::new(None, false)
        }

        fn failing_transition() -> Self {
            Self::new(Some(RuntimeKind::Runpod), true)
        }

        fn new(runtime: Option<RuntimeKind>, fail: bool) -> Self {
            let committed = Arc::new(AtomicBool::new(false));
            let workspace = Workspace {
                id: "workspace-1".into(),
                workflow: CatalogRef::new("workflow-1", "1"),
                created_at: OffsetDateTime::UNIX_EPOCH,
                runtime,
            };
            Self {
                transitions: Arc::new(FakeTransitionRepository {
                    committed: committed.clone(),
                    fail,
                }),
                workspaces: Arc::new(FakeWorkspaceRepository(workspace.clone())),
                events: Arc::new(RecordingEventSink {
                    committed,
                    events: Mutex::new(Vec::new()),
                }),
                runtime: RunpodRuntime {
                    workspace_id: "workspace-1".into(),
                    state: RunpodRuntimeState::Ready,
                    config: RunpodRuntimeConfig {
                        datacenter_id: "dc-1".into(),
                        gpu_id: "gpu-1".into(),
                        volume_size_gb: 19,
                    },
                    resources: RunpodRuntimeResources::default(),
                },
                operation: RuntimeOperation::running(
                    Uuid::from_u128(1),
                    "workspace-1",
                    Uuid::from_u128(2),
                    RuntimeOperationKind::Provision,
                    RuntimeProgress::Runpod(RunpodProgress::Provision(
                        RunpodProvisionStep::CreateEndpoint,
                    )),
                    OffsetDateTime::UNIX_EPOCH,
                ),
                workspace,
            }
        }

        fn context(&self) -> RuntimeTransitionContext<RunpodRuntime, FakeTransitionRepository> {
            RuntimeTransitionContext::new(
                self.transitions.clone(),
                self.workspaces.clone(),
                self.events.clone(),
            )
        }
    }

    struct FakeTransitionRepository {
        committed: Arc<AtomicBool>,
        fail: bool,
    }

    #[async_trait::async_trait]
    impl RuntimeTransitionRepository<RunpodRuntime> for FakeTransitionRepository {
        async fn save_transition(
            &self,
            _: &RunpodRuntime,
            _: &RuntimeOperation,
        ) -> Result<(), RuntimeTransitionRepositoryError> {
            if self.fail {
                return Err(RuntimeTransitionRepositoryError::Unavailable);
            }
            self.committed.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    struct RecordingEventSink {
        committed: Arc<AtomicBool>,
        events: Mutex<Vec<(bool, ApplicationEvent)>>,
    }

    impl RecordingEventSink {
        fn events(&self) -> Vec<ApplicationEvent> {
            self.events
                .lock()
                .unwrap()
                .iter()
                .map(|(_, event)| event.clone())
                .collect()
        }

        fn all_emitted_after_commit(&self) -> bool {
            self.events
                .lock()
                .unwrap()
                .iter()
                .all(|(committed, _)| *committed)
        }
    }

    impl ApplicationEventSink for RecordingEventSink {
        fn emit(&self, event: ApplicationEvent) {
            self.events
                .lock()
                .unwrap()
                .push((self.committed.load(Ordering::SeqCst), event));
        }
    }

    struct FakeWorkspaceRepository(Workspace);

    #[async_trait::async_trait]
    impl WorkspaceRepository for FakeWorkspaceRepository {
        async fn create(&self, _: Workspace) -> Result<Workspace, WorkspaceRepositoryError> {
            unreachable!()
        }

        async fn get(&self, id: &str) -> Result<Option<Workspace>, WorkspaceRepositoryError> {
            Ok((self.0.id == id).then(|| self.0.clone()))
        }

        async fn list(&self) -> Result<Vec<Workspace>, WorkspaceRepositoryError> {
            unreachable!()
        }

        async fn delete(&self, _: &str) -> Result<bool, WorkspaceRepositoryError> {
            unreachable!()
        }
    }

    struct BlockingWorkspaceRepository {
        workspace: Workspace,
        gets: AtomicUsize,
        projection_entered: Mutex<Option<oneshot::Sender<()>>>,
        release_projection: Mutex<Option<oneshot::Receiver<()>>>,
    }

    #[async_trait::async_trait]
    impl WorkspaceRepository for BlockingWorkspaceRepository {
        async fn create(&self, _: Workspace) -> Result<Workspace, WorkspaceRepositoryError> {
            unreachable!()
        }

        async fn get(&self, id: &str) -> Result<Option<Workspace>, WorkspaceRepositoryError> {
            if self.gets.fetch_add(1, Ordering::SeqCst) == 0 {
                self.projection_entered
                    .lock()
                    .unwrap()
                    .take()
                    .unwrap()
                    .send(())
                    .unwrap();
                let release = self.release_projection.lock().unwrap().take().unwrap();
                release.await.unwrap();
            }
            Ok((self.workspace.id == id).then(|| self.workspace.clone()))
        }

        async fn list(&self) -> Result<Vec<Workspace>, WorkspaceRepositoryError> {
            unreachable!()
        }

        async fn delete(&self, _: &str) -> Result<bool, WorkspaceRepositoryError> {
            unreachable!()
        }
    }
}
