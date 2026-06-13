use std::{future::Future, sync::Arc, time::Duration};

use crate::{
    domain::lifecycle_operation::{LifecycleOperationId, LifecycleOperationState},
    lifecycle_journal::LifecycleJournalRepository,
    runpod_runtime::{
        errors::RunpodRuntimeError,
        events::{RunpodRuntimeEvent, RunpodRuntimeEventSink},
        provider::RunpodRuntimeClient,
    },
    shared::{spawn_background_task, BackgroundTaskSpawner, InFlightRegistry},
    workflow_catalog::WorkflowCatalogService,
    workspace_catalog::WorkspaceCatalogRepository,
};

use super::{cleanup, delete, provision};

pub(crate) type LifecycleOperationRegistry = InFlightRegistry<LifecycleOperationId>;
const PROVISIONER_POLL_INTERVAL: Duration = Duration::from_secs(5);

pub(crate) struct RunpodRuntimeLifecycleRunnerContext<W, L>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
{
    pub(crate) workspace_repository: W,
    pub(crate) lifecycle_journal: L,
    pub(crate) workflow_catalog: WorkflowCatalogService,
    pub(crate) runpod_client: Arc<dyn RunpodRuntimeClient>,
    pub(crate) lifecycle_operation_registry: LifecycleOperationRegistry,
    pub(crate) event_sink: Arc<dyn RunpodRuntimeEventSink>,
    pub(crate) task_spawner: Arc<dyn BackgroundTaskSpawner>,
}

pub(crate) trait RunpodRuntimeLifecycleRunner<W, L>: Send + Sync
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
{
    fn spawn_provision(
        &self,
        context: RunpodRuntimeLifecycleRunnerContext<W, L>,
        operation_id: LifecycleOperationId,
    );

    fn spawn_cleanup(
        &self,
        context: RunpodRuntimeLifecycleRunnerContext<W, L>,
        operation_id: LifecycleOperationId,
    );

    fn spawn_delete(
        &self,
        context: RunpodRuntimeLifecycleRunnerContext<W, L>,
        operation_id: LifecycleOperationId,
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BackgroundRunpodRuntimeLifecycleRunner;

impl<W, L> RunpodRuntimeLifecycleRunner<W, L> for BackgroundRunpodRuntimeLifecycleRunner
where
    W: WorkspaceCatalogRepository + Clone + Send + Sync + 'static,
    L: LifecycleJournalRepository + Clone + Send + Sync + 'static,
{
    fn spawn_provision(
        &self,
        context: RunpodRuntimeLifecycleRunnerContext<W, L>,
        operation_id: LifecycleOperationId,
    ) {
        if !context
            .lifecycle_operation_registry
            .try_register(&operation_id)
        {
            return;
        }

        let registry = context.lifecycle_operation_registry.clone();
        let workspace_repository = context.workspace_repository;
        let lifecycle_journal = context.lifecycle_journal;
        let workflow_catalog = context.workflow_catalog;
        let runpod_client = context.runpod_client;
        let event_sink = context.event_sink;
        spawn_lifecycle_runner(
            context.task_spawner.as_ref(),
            registry,
            lifecycle_journal.clone(),
            event_sink.clone(),
            operation_id.clone(),
            async move {
                provision::run_once(
                    &operation_id,
                    &workspace_repository,
                    &lifecycle_journal,
                    &workflow_catalog,
                    runpod_client.as_ref(),
                    &event_sink,
                    PROVISIONER_POLL_INTERVAL,
                )
                .await
            },
        );
    }

    fn spawn_cleanup(
        &self,
        context: RunpodRuntimeLifecycleRunnerContext<W, L>,
        operation_id: LifecycleOperationId,
    ) {
        if !context
            .lifecycle_operation_registry
            .try_register(&operation_id)
        {
            return;
        }

        let registry = context.lifecycle_operation_registry.clone();
        let workspace_repository = context.workspace_repository;
        let lifecycle_journal = context.lifecycle_journal;
        let runpod_client = context.runpod_client;
        let event_sink = context.event_sink;
        spawn_lifecycle_runner(
            context.task_spawner.as_ref(),
            registry,
            lifecycle_journal.clone(),
            event_sink.clone(),
            operation_id.clone(),
            async move {
                cleanup::run_once(
                    &operation_id,
                    &workspace_repository,
                    &lifecycle_journal,
                    runpod_client.as_ref(),
                    &event_sink,
                )
                .await
            },
        );
    }

    fn spawn_delete(
        &self,
        context: RunpodRuntimeLifecycleRunnerContext<W, L>,
        operation_id: LifecycleOperationId,
    ) {
        if !context
            .lifecycle_operation_registry
            .try_register(&operation_id)
        {
            return;
        }

        let registry = context.lifecycle_operation_registry.clone();
        let workspace_repository = context.workspace_repository;
        let lifecycle_journal = context.lifecycle_journal;
        let runpod_client = context.runpod_client;
        let event_sink = context.event_sink;
        spawn_lifecycle_runner(
            context.task_spawner.as_ref(),
            registry,
            lifecycle_journal.clone(),
            event_sink.clone(),
            operation_id.clone(),
            async move {
                delete::run_once(
                    &operation_id,
                    &workspace_repository,
                    &lifecycle_journal,
                    runpod_client.as_ref(),
                    &event_sink,
                )
                .await
            },
        );
    }
}

fn spawn_lifecycle_runner<F, T>(
    task_spawner: &dyn BackgroundTaskSpawner,
    registry: LifecycleOperationRegistry,
    lifecycle_journal: impl LifecycleJournalRepository + 'static,
    event_sink: Arc<dyn RunpodRuntimeEventSink>,
    operation_id: LifecycleOperationId,
    lifecycle: F,
) where
    F: Future<Output = Result<T, RunpodRuntimeError>> + Send + 'static,
    T: Send + 'static,
{
    spawn_background_task(task_spawner, async move {
        if let Err(error) = lifecycle.await {
            record_lifecycle_runner_error(&lifecycle_journal, &event_sink, &operation_id, &error)
                .await;
        }
        registry.complete(&operation_id);
    });
}

async fn record_lifecycle_runner_error<L>(
    lifecycle_journal: &L,
    event_sink: &Arc<dyn RunpodRuntimeEventSink>,
    operation_id: &LifecycleOperationId,
    error: &RunpodRuntimeError,
) where
    L: LifecycleJournalRepository,
{
    let operations = match lifecycle_journal.list_running().await {
        Ok(operations) => operations,
        Err(_) => return,
    };
    let Some(operation) = operations
        .into_iter()
        .find(|operation| operation.operation_id == *operation_id)
    else {
        return;
    };

    let diagnostic_id =
        crate::diagnostics::lifecycle_error(operation_id, Some(&operation.workspace_id), error);

    let failed_operation = match lifecycle_journal
        .mark_state(
            operation_id,
            LifecycleOperationState::Failed,
            &operation.payload,
        )
        .await
    {
        Ok(operation) => operation,
        Err(_) => return,
    };

    event_sink.emit(RunpodRuntimeEvent::LifecycleOperationChanged {
        workspace_id: failed_operation.workspace_id.clone(),
        operation_id: failed_operation.operation_id.clone(),
        diagnostic_id: Some(diagnostic_id),
        operation: failed_operation,
    });
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    use crate::{
        domain::{
            lifecycle_operation::{
                LifecycleOperation, LifecycleOperationId, LifecycleOperationPayload,
                LifecycleOperationState,
            },
            runpod::RunpodLifecycleOperationPayload,
            workspace::WorkspaceId,
        },
        lifecycle_journal::{LifecycleJournalError, LifecycleJournalRepository},
        runpod_runtime::{errors::RunpodRuntimeError, events::RunpodRuntimeEvent},
        shared::{AppFuture, BackgroundTask, BackgroundTaskSpawner, EventSink},
    };
    use time::OffsetDateTime;

    use super::{spawn_lifecycle_runner, LifecycleOperationRegistry};

    struct TokioTaskSpawner;

    impl BackgroundTaskSpawner for TokioTaskSpawner {
        fn spawn(&self, task: BackgroundTask) {
            tokio::spawn(task);
        }
    }

    #[tokio::test]
    async fn failed_lifecycle_runner_clears_in_flight_operation() {
        let registry = LifecycleOperationRegistry::default();
        let operation_id = "operation-1".to_string();
        let lifecycle_journal = FakeLifecycleJournal::new(operation_id.clone());
        let event_sink = Arc::new(FakeEventSink::default());
        assert!(registry.try_register(&operation_id));

        spawn_lifecycle_runner(
            &TokioTaskSpawner,
            registry.clone(),
            lifecycle_journal.clone(),
            event_sink.clone(),
            operation_id.clone(),
            async {
                Err::<(), _>(RunpodRuntimeError::InvalidRuntimeState {
                    message: "runner failed".to_string(),
                })
            },
        );

        for _ in 0..10 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            if registry.try_register(&operation_id) {
                assert_eq!(
                    lifecycle_journal.operation_state(),
                    LifecycleOperationState::Failed
                );
                assert_eq!(event_sink.event_count(), 1);
                return;
            }
        }

        assert!(
            registry.try_register(&operation_id),
            "failed lifecycle runner should clear in-flight registry"
        );
    }

    #[derive(Clone)]
    struct FakeLifecycleJournal {
        operation: Arc<Mutex<LifecycleOperation>>,
    }

    impl FakeLifecycleJournal {
        fn new(operation_id: String) -> Self {
            Self {
                operation: Arc::new(Mutex::new(LifecycleOperation {
                    operation_id,
                    workspace_id: "workspace-1".to_string(),
                    state: LifecycleOperationState::Running,
                    payload: LifecycleOperationPayload::Runpod(
                        RunpodLifecycleOperationPayload::Provision { step: None },
                    ),
                    created_at: OffsetDateTime::UNIX_EPOCH,
                    updated_at: OffsetDateTime::UNIX_EPOCH,
                    finished_at: None,
                })),
            }
        }

        fn operation_state(&self) -> LifecycleOperationState {
            self.operation.lock().expect("operation state").state
        }
    }

    impl LifecycleJournalRepository for FakeLifecycleJournal {
        fn create_operation<'a>(
            &'a self,
            _workspace_id: &'a WorkspaceId,
            _payload: &'a LifecycleOperationPayload,
        ) -> AppFuture<'a, Result<LifecycleOperation, LifecycleJournalError>> {
            Box::pin(async { Err(LifecycleJournalError::OperationNotFound) })
        }

        fn find_running_by_workspace<'a>(
            &'a self,
            _workspace_id: &'a WorkspaceId,
        ) -> AppFuture<'a, Result<Option<LifecycleOperation>, LifecycleJournalError>> {
            Box::pin(async { Ok(None) })
        }

        fn list_running<'a>(
            &'a self,
        ) -> AppFuture<'a, Result<Vec<LifecycleOperation>, LifecycleJournalError>> {
            Box::pin(async move {
                Ok(vec![self
                    .operation
                    .lock()
                    .expect("operation state")
                    .clone()])
            })
        }

        fn latest_for_workspace<'a>(
            &'a self,
            _workspace_id: &'a WorkspaceId,
        ) -> AppFuture<'a, Result<Option<LifecycleOperation>, LifecycleJournalError>> {
            Box::pin(async { Ok(None) })
        }

        fn delete_for_workspace<'a>(
            &'a self,
            _workspace_id: &'a WorkspaceId,
        ) -> AppFuture<'a, Result<(), LifecycleJournalError>> {
            Box::pin(async { Ok(()) })
        }

        fn update_operation<'a>(
            &'a self,
            _operation: &'a LifecycleOperation,
        ) -> AppFuture<'a, Result<LifecycleOperation, LifecycleJournalError>> {
            Box::pin(async { Err(LifecycleJournalError::OperationNotFound) })
        }

        fn mark_state<'a>(
            &'a self,
            _operation_id: &'a LifecycleOperationId,
            state: LifecycleOperationState,
            payload: &'a LifecycleOperationPayload,
        ) -> AppFuture<'a, Result<LifecycleOperation, LifecycleJournalError>> {
            Box::pin(async move {
                let mut operation = self.operation.lock().expect("operation state");
                operation.state = state;
                operation.payload = payload.clone();
                Ok(operation.clone())
            })
        }
    }

    #[derive(Default)]
    struct FakeEventSink {
        events: Mutex<Vec<RunpodRuntimeEvent>>,
    }

    impl FakeEventSink {
        fn event_count(&self) -> usize {
            self.events.lock().expect("event state").len()
        }
    }

    impl EventSink<RunpodRuntimeEvent> for FakeEventSink {
        fn emit(&self, event: RunpodRuntimeEvent) {
            self.events.lock().expect("event state").push(event);
        }
    }
}
