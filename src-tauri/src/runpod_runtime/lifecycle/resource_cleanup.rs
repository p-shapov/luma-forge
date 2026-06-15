use std::sync::Arc;

use crate::{
    domain::{
        lifecycle_operation::LifecycleOperation,
        runpod::{RunpodCleanupStep, RunpodDeleteStep, RunpodResources},
        workspace::{Workspace, WorkspaceRuntime},
    },
    lifecycle_journal::LifecycleJournalRepository,
    workspace_catalog::WorkspaceCatalogRepository,
};

use super::{
    super::{
        errors::RunpodRuntimeError, events::RunpodRuntimeEventSink, provider::RunpodRuntimeClient,
    },
    helpers::{mark_running_step, persist_workspace, RunpodStepPayload},
};

pub(super) trait RemoteResourceCleanupStep: Clone + RunpodStepPayload {
    fn delete_endpoint() -> Self;
    fn delete_template() -> Self;
    fn terminate_provisioner_pod() -> Self;
    fn delete_network_volume() -> Self;
}

impl RemoteResourceCleanupStep for RunpodCleanupStep {
    fn delete_endpoint() -> Self {
        Self::DeleteEndpoint
    }

    fn delete_template() -> Self {
        Self::DeleteTemplate
    }

    fn terminate_provisioner_pod() -> Self {
        Self::TerminateProvisionerPod
    }

    fn delete_network_volume() -> Self {
        Self::DeleteNetworkVolume
    }
}

impl RemoteResourceCleanupStep for RunpodDeleteStep {
    fn delete_endpoint() -> Self {
        Self::DeleteEndpoint
    }

    fn delete_template() -> Self {
        Self::DeleteTemplate
    }

    fn terminate_provisioner_pod() -> Self {
        Self::TerminateProvisionerPod
    }

    fn delete_network_volume() -> Self {
        Self::DeleteNetworkVolume
    }
}

#[tracing::instrument(
    name = "runpod_lifecycle_cleanup",
    skip_all,
    fields(
        operation_id = %operation.operation_id,
        workspace_id = %operation.workspace_id
    )
)]
pub(super) async fn delete_remote_resources<W, L, S>(
    workspace: &mut Workspace,
    workspace_catalog: &W,
    lifecycle_journal: &L,
    operation: &LifecycleOperation,
    runpod_client: &dyn RunpodRuntimeClient,
    event_sink: &Arc<dyn RunpodRuntimeEventSink>,
    failed_step: &mut S,
) -> Result<(), RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
    S: RemoteResourceCleanupStep,
{
    delete_endpoint(
        workspace,
        workspace_catalog,
        lifecycle_journal,
        operation,
        runpod_client,
        event_sink,
        failed_step,
    )
    .await?;
    delete_template(
        workspace,
        workspace_catalog,
        lifecycle_journal,
        operation,
        runpod_client,
        event_sink,
        failed_step,
    )
    .await?;
    terminate_provisioner_pod(
        workspace,
        workspace_catalog,
        lifecycle_journal,
        operation,
        runpod_client,
        event_sink,
        failed_step,
    )
    .await?;
    delete_network_volume(
        workspace,
        workspace_catalog,
        lifecycle_journal,
        operation,
        runpod_client,
        event_sink,
        failed_step,
    )
    .await
}

#[tracing::instrument(
    name = "runpod_lifecycle_step",
    skip_all,
    fields(
        step = "delete_endpoint",
        operation_id = %operation.operation_id,
        workspace_id = %operation.workspace_id
    )
)]
async fn delete_endpoint<W, L, S>(
    workspace: &mut Workspace,
    workspace_catalog: &W,
    lifecycle_journal: &L,
    operation: &LifecycleOperation,
    runpod_client: &dyn RunpodRuntimeClient,
    event_sink: &Arc<dyn RunpodRuntimeEventSink>,
    failed_step: &mut S,
) -> Result<(), RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
    S: RemoteResourceCleanupStep,
{
    let Some(endpoint_id) = resources(workspace).endpoint_id.clone() else {
        return Ok(());
    };

    *failed_step = S::delete_endpoint();
    mark_running_step(
        lifecycle_journal,
        event_sink,
        operation,
        failed_step.clone(),
    )
    .await?;

    runpod_client
        .delete_serverless_endpoint(&endpoint_id)
        .await?;
    resources_mut(workspace).endpoint_id = None;
    *workspace = persist_workspace(workspace_catalog, event_sink, workspace).await?;
    Ok(())
}

#[tracing::instrument(
    name = "runpod_lifecycle_step",
    skip_all,
    fields(
        step = "delete_template",
        operation_id = %operation.operation_id,
        workspace_id = %operation.workspace_id
    )
)]
async fn delete_template<W, L, S>(
    workspace: &mut Workspace,
    workspace_catalog: &W,
    lifecycle_journal: &L,
    operation: &LifecycleOperation,
    runpod_client: &dyn RunpodRuntimeClient,
    event_sink: &Arc<dyn RunpodRuntimeEventSink>,
    failed_step: &mut S,
) -> Result<(), RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
    S: RemoteResourceCleanupStep,
{
    let Some(template_id) = resources(workspace).template_id.clone() else {
        return Ok(());
    };

    *failed_step = S::delete_template();
    mark_running_step(
        lifecycle_journal,
        event_sink,
        operation,
        failed_step.clone(),
    )
    .await?;

    runpod_client.delete_template(&template_id).await?;
    resources_mut(workspace).template_id = None;
    *workspace = persist_workspace(workspace_catalog, event_sink, workspace).await?;
    Ok(())
}

#[tracing::instrument(
    name = "runpod_lifecycle_step",
    skip_all,
    fields(
        step = "terminate_provisioner_pod",
        operation_id = %operation.operation_id,
        workspace_id = %operation.workspace_id
    )
)]
async fn terminate_provisioner_pod<W, L, S>(
    workspace: &mut Workspace,
    workspace_catalog: &W,
    lifecycle_journal: &L,
    operation: &LifecycleOperation,
    runpod_client: &dyn RunpodRuntimeClient,
    event_sink: &Arc<dyn RunpodRuntimeEventSink>,
    failed_step: &mut S,
) -> Result<(), RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
    S: RemoteResourceCleanupStep,
{
    let Some(provisioner_id) = resources(workspace).provisioner_pod_id.clone() else {
        return Ok(());
    };

    *failed_step = S::terminate_provisioner_pod();
    mark_running_step(
        lifecycle_journal,
        event_sink,
        operation,
        failed_step.clone(),
    )
    .await?;

    runpod_client
        .terminate_provisioner_pod(&provisioner_id)
        .await?;
    resources_mut(workspace).provisioner_pod_id = None;
    *workspace = persist_workspace(workspace_catalog, event_sink, workspace).await?;
    Ok(())
}

#[tracing::instrument(
    name = "runpod_lifecycle_step",
    skip_all,
    fields(
        step = "delete_network_volume",
        operation_id = %operation.operation_id,
        workspace_id = %operation.workspace_id
    )
)]
async fn delete_network_volume<W, L, S>(
    workspace: &mut Workspace,
    workspace_catalog: &W,
    lifecycle_journal: &L,
    operation: &LifecycleOperation,
    runpod_client: &dyn RunpodRuntimeClient,
    event_sink: &Arc<dyn RunpodRuntimeEventSink>,
    failed_step: &mut S,
) -> Result<(), RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
    S: RemoteResourceCleanupStep,
{
    let Some(volume_id) = resources(workspace).network_volume_id.clone() else {
        return Ok(());
    };

    *failed_step = S::delete_network_volume();
    mark_running_step(
        lifecycle_journal,
        event_sink,
        operation,
        failed_step.clone(),
    )
    .await?;

    runpod_client.delete_network_volume(&volume_id).await?;
    resources_mut(workspace).network_volume_id = None;
    *workspace = persist_workspace(workspace_catalog, event_sink, workspace).await?;
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
