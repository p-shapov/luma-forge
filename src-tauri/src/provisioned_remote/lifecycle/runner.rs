use std::{sync::Arc, time::Duration};

use crate::{
    domain::lifecycle_operation::LifecycleOperationId,
    lifecycle_journal::LifecycleJournalRepository,
    provisioned_remote::{
        events::ProvisionedRemoteEventSink, registry::ProvisionedRemoteProviderRegistry,
    },
    shared::{spawn_background_task, BackgroundTaskSpawner, InFlightRegistry},
    workspace_catalog::WorkspaceCatalogRepository,
};

use super::{cleanup, delete, provision};

pub(crate) type LifecycleOperationRegistry = InFlightRegistry<LifecycleOperationId>;
const PROVISIONER_POLL_INTERVAL: Duration = Duration::from_secs(5);

pub(crate) struct ProvisionedRemoteLifecycleRunnerContext<W, L>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
{
    pub(crate) workspace_repository: W,
    pub(crate) lifecycle_journal: L,
    pub(crate) provider_registry: ProvisionedRemoteProviderRegistry,
    pub(crate) lifecycle_operation_registry: LifecycleOperationRegistry,
    pub(crate) event_sink: Arc<dyn ProvisionedRemoteEventSink>,
    pub(crate) task_spawner: Arc<dyn BackgroundTaskSpawner>,
}

pub(crate) trait ProvisionedRemoteLifecycleRunner<W, L>: Send + Sync
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
{
    fn spawn_provision(
        &self,
        context: ProvisionedRemoteLifecycleRunnerContext<W, L>,
        operation_id: LifecycleOperationId,
    );

    fn spawn_cleanup(
        &self,
        context: ProvisionedRemoteLifecycleRunnerContext<W, L>,
        operation_id: LifecycleOperationId,
    );

    fn spawn_delete(
        &self,
        context: ProvisionedRemoteLifecycleRunnerContext<W, L>,
        operation_id: LifecycleOperationId,
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BackgroundProvisionedRemoteLifecycleRunner;

impl<W, L> ProvisionedRemoteLifecycleRunner<W, L> for BackgroundProvisionedRemoteLifecycleRunner
where
    W: WorkspaceCatalogRepository + Clone + Send + Sync + 'static,
    L: LifecycleJournalRepository + Clone + Send + Sync + 'static,
{
    fn spawn_provision(
        &self,
        context: ProvisionedRemoteLifecycleRunnerContext<W, L>,
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
        let provider_registry = context.provider_registry;
        let event_sink = context.event_sink;
        spawn_background_task(context.task_spawner.as_ref(), async move {
            let _ = provision::run_once(
                &operation_id,
                &workspace_repository,
                &lifecycle_journal,
                &provider_registry,
                &event_sink,
                PROVISIONER_POLL_INTERVAL,
            )
            .await;
            registry.complete(&operation_id);
        });
    }

    fn spawn_cleanup(
        &self,
        context: ProvisionedRemoteLifecycleRunnerContext<W, L>,
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
        let provider_registry = context.provider_registry;
        let event_sink = context.event_sink;
        spawn_background_task(context.task_spawner.as_ref(), async move {
            let _ = cleanup::run_once(
                &operation_id,
                &workspace_repository,
                &lifecycle_journal,
                &provider_registry,
                &event_sink,
            )
            .await;
            registry.complete(&operation_id);
        });
    }

    fn spawn_delete(
        &self,
        context: ProvisionedRemoteLifecycleRunnerContext<W, L>,
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
        let provider_registry = context.provider_registry;
        let event_sink = context.event_sink;
        spawn_background_task(context.task_spawner.as_ref(), async move {
            let _ = delete::run_once(
                &operation_id,
                &workspace_repository,
                &lifecycle_journal,
                &provider_registry,
                &event_sink,
            )
            .await;
            registry.complete(&operation_id);
        });
    }
}
