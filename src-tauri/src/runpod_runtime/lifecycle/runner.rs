use std::{future::Future, sync::Arc, time::Duration};

use crate::{
    domain::lifecycle_operation::LifecycleOperationId,
    lifecycle_journal::LifecycleJournalRepository,
    runpod_runtime::{
        errors::RunpodRuntimeError, events::RunpodRuntimeEventSink, provider::RunpodRuntimeClient,
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
    operation_id: LifecycleOperationId,
    lifecycle: F,
) where
    F: Future<Output = Result<T, RunpodRuntimeError>> + Send + 'static,
    T: Send + 'static,
{
    spawn_background_task(task_spawner, async move {
        if lifecycle.await.is_ok() {
            registry.complete(&operation_id);
        }
    });
}
