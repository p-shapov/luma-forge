use std::sync::Arc;

use crate::{
    domain::{
        lifecycle_operation::LifecycleOperation,
        runpod::RunpodResources,
        workspace::{Workspace, WorkspaceRuntime},
    },
    lifecycle_journal::LifecycleJournalRepository,
    shared::EventSink,
    workspace_catalog::WorkspaceCatalogRepository,
};

use super::{
    super::{
        errors::RunpodRuntimeError, events::RunpodRuntimeEvent, provider::RunpodRuntimeClient,
    },
    helpers::{mark_running_step, persist_workspace, RunpodStepPayload},
};

#[derive(Debug, Clone)]
pub(super) struct RemoteResourceCleanupSteps<S> {
    pub(super) delete_endpoint: S,
    pub(super) delete_template: S,
    pub(super) terminate_provisioner_pod: S,
    pub(super) delete_network_volume: S,
}

pub(super) struct RemoteResourceCleanupContext<'a, W, L>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
{
    pub(super) workspace_catalog: &'a W,
    pub(super) lifecycle_journal: &'a L,
    pub(super) operation: &'a LifecycleOperation,
    pub(super) runpod_client: &'a dyn RunpodRuntimeClient,
    pub(super) event_sink: &'a Arc<dyn EventSink<RunpodRuntimeEvent>>,
}

#[tracing::instrument(
    name = "runpod_lifecycle_cleanup",
    skip_all,
    fields(
        operation_id = %context.operation.operation_id,
        workspace_id = %context.operation.workspace_id
    )
)]
pub(super) async fn delete_remote_resources<W, L, S>(
    workspace: &mut Workspace,
    context: RemoteResourceCleanupContext<'_, W, L>,
    steps: RemoteResourceCleanupSteps<S>,
    failed_step: &mut S,
) -> Result<(), RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
    S: Clone + RunpodStepPayload,
{
    delete_endpoint(workspace, &context, steps.delete_endpoint, failed_step).await?;
    delete_template(workspace, &context, steps.delete_template, failed_step).await?;
    terminate_provisioner_pod(
        workspace,
        &context,
        steps.terminate_provisioner_pod,
        failed_step,
    )
    .await?;
    delete_network_volume(
        workspace,
        &context,
        steps.delete_network_volume,
        failed_step,
    )
    .await
}

#[tracing::instrument(
    name = "runpod_lifecycle_step",
    skip_all,
    fields(
        step = "delete_endpoint",
        operation_id = %context.operation.operation_id,
        workspace_id = %context.operation.workspace_id
    )
)]
async fn delete_endpoint<W, L, S>(
    workspace: &mut Workspace,
    context: &RemoteResourceCleanupContext<'_, W, L>,
    step: S,
    failed_step: &mut S,
) -> Result<(), RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
    S: Clone + RunpodStepPayload,
{
    let Some(endpoint_id) = resources(workspace).endpoint_id.clone() else {
        return Ok(());
    };

    *failed_step = step;
    mark_running_step(
        context.lifecycle_journal,
        context.event_sink,
        context.operation,
        failed_step.clone(),
    )
    .await?;

    context
        .runpod_client
        .delete_serverless_endpoint(&endpoint_id)
        .await?;
    resources_mut(workspace).endpoint_id = None;
    *workspace =
        persist_workspace(context.workspace_catalog, context.event_sink, workspace).await?;
    Ok(())
}

#[tracing::instrument(
    name = "runpod_lifecycle_step",
    skip_all,
    fields(
        step = "delete_template",
        operation_id = %context.operation.operation_id,
        workspace_id = %context.operation.workspace_id
    )
)]
async fn delete_template<W, L, S>(
    workspace: &mut Workspace,
    context: &RemoteResourceCleanupContext<'_, W, L>,
    step: S,
    failed_step: &mut S,
) -> Result<(), RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
    S: Clone + RunpodStepPayload,
{
    let Some(template_id) = resources(workspace).template_id.clone() else {
        return Ok(());
    };

    *failed_step = step;
    mark_running_step(
        context.lifecycle_journal,
        context.event_sink,
        context.operation,
        failed_step.clone(),
    )
    .await?;

    context.runpod_client.delete_template(&template_id).await?;
    resources_mut(workspace).template_id = None;
    *workspace =
        persist_workspace(context.workspace_catalog, context.event_sink, workspace).await?;
    Ok(())
}

#[tracing::instrument(
    name = "runpod_lifecycle_step",
    skip_all,
    fields(
        step = "terminate_provisioner_pod",
        operation_id = %context.operation.operation_id,
        workspace_id = %context.operation.workspace_id
    )
)]
async fn terminate_provisioner_pod<W, L, S>(
    workspace: &mut Workspace,
    context: &RemoteResourceCleanupContext<'_, W, L>,
    step: S,
    failed_step: &mut S,
) -> Result<(), RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
    S: Clone + RunpodStepPayload,
{
    let Some(provisioner_id) = resources(workspace).provisioner_pod_id.clone() else {
        return Ok(());
    };

    *failed_step = step;
    mark_running_step(
        context.lifecycle_journal,
        context.event_sink,
        context.operation,
        failed_step.clone(),
    )
    .await?;

    context
        .runpod_client
        .terminate_provisioner_pod(&provisioner_id)
        .await?;
    resources_mut(workspace).provisioner_pod_id = None;
    *workspace =
        persist_workspace(context.workspace_catalog, context.event_sink, workspace).await?;
    Ok(())
}

#[tracing::instrument(
    name = "runpod_lifecycle_step",
    skip_all,
    fields(
        step = "delete_network_volume",
        operation_id = %context.operation.operation_id,
        workspace_id = %context.operation.workspace_id
    )
)]
async fn delete_network_volume<W, L, S>(
    workspace: &mut Workspace,
    context: &RemoteResourceCleanupContext<'_, W, L>,
    step: S,
    failed_step: &mut S,
) -> Result<(), RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
    S: Clone + RunpodStepPayload,
{
    let Some(volume_id) = resources(workspace).network_volume_id.clone() else {
        return Ok(());
    };

    *failed_step = step;
    mark_running_step(
        context.lifecycle_journal,
        context.event_sink,
        context.operation,
        failed_step.clone(),
    )
    .await?;

    context
        .runpod_client
        .delete_network_volume(&volume_id)
        .await?;
    resources_mut(workspace).network_volume_id = None;
    *workspace =
        persist_workspace(context.workspace_catalog, context.event_sink, workspace).await?;
    Ok(())
}

fn resources(workspace: &Workspace) -> &RunpodResources {
    let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
    &runtime.resources
}

fn resources_mut(workspace: &mut Workspace) -> &mut RunpodResources {
    let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
    &mut runtime.resources
}
