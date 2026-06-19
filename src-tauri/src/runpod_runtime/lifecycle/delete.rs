use std::sync::Arc;

use crate::{shared::EventSink, workspace_catalog::WorkspaceCatalogRepository};

use super::{
    super::{
        errors::RunpodRuntimeError, events::RunpodRuntimeEvent, provider::RunpodRuntimeClient,
    },
    helpers::{mark_workspace_failed, persist_workspace},
};

#[tracing::instrument(
    name = "runpod_delete_workspace",
    skip_all,
    fields(workspace_id = %workspace.id)
)]
pub async fn delete_workspace<W>(
    mut workspace: crate::domain::workspace::Workspace,
    workspace_catalog: &W,
    runpod_client: &dyn RunpodRuntimeClient,
    event_sink: &Arc<dyn EventSink<RunpodRuntimeEvent>>,
) -> Result<(), RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
{
    let result = async {
        delete_endpoint(&mut workspace, workspace_catalog, runpod_client, event_sink).await?;
        delete_template(&mut workspace, workspace_catalog, runpod_client, event_sink).await?;
        terminate_provisioner_pod(&mut workspace, workspace_catalog, runpod_client, event_sink)
            .await?;
        delete_network_volume(&mut workspace, workspace_catalog, runpod_client, event_sink).await?;
        workspace_catalog
            .delete_workspace(&workspace.id)
            .await
            .map_err(RunpodRuntimeError::from)?;
        event_sink.emit(RunpodRuntimeEvent::WorkspaceDeleted {
            workspace_id: workspace.id.clone(),
        });
        Ok::<(), RunpodRuntimeError>(())
    }
    .await;

    match result {
        Ok(()) => Ok(()),
        Err(error) => {
            mark_workspace_failed(&mut workspace, workspace_catalog, event_sink).await?;
            Err(error)
        }
    }
}

async fn delete_endpoint<W>(
    workspace: &mut crate::domain::workspace::Workspace,
    workspace_catalog: &W,
    runpod_client: &dyn RunpodRuntimeClient,
    event_sink: &Arc<dyn EventSink<RunpodRuntimeEvent>>,
) -> Result<(), RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
{
    let Some(endpoint_id) = runpod_resources(workspace).endpoint_id.clone() else {
        return Ok(());
    };
    runpod_client
        .delete_serverless_endpoint(&endpoint_id)
        .await?;
    runpod_resources_mut(workspace).endpoint_id = None;
    *workspace = persist_workspace(workspace_catalog, event_sink, workspace).await?;
    Ok(())
}

async fn delete_template<W>(
    workspace: &mut crate::domain::workspace::Workspace,
    workspace_catalog: &W,
    runpod_client: &dyn RunpodRuntimeClient,
    event_sink: &Arc<dyn EventSink<RunpodRuntimeEvent>>,
) -> Result<(), RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
{
    let Some(template_id) = runpod_resources(workspace).template_id.clone() else {
        return Ok(());
    };
    runpod_client.delete_template(&template_id).await?;
    runpod_resources_mut(workspace).template_id = None;
    *workspace = persist_workspace(workspace_catalog, event_sink, workspace).await?;
    Ok(())
}

async fn terminate_provisioner_pod<W>(
    workspace: &mut crate::domain::workspace::Workspace,
    workspace_catalog: &W,
    runpod_client: &dyn RunpodRuntimeClient,
    event_sink: &Arc<dyn EventSink<RunpodRuntimeEvent>>,
) -> Result<(), RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
{
    let Some(provisioner_id) = runpod_resources(workspace).provisioner_pod_id.clone() else {
        return Ok(());
    };
    runpod_client
        .terminate_provisioner_pod(&provisioner_id)
        .await?;
    runpod_resources_mut(workspace).provisioner_pod_id = None;
    *workspace = persist_workspace(workspace_catalog, event_sink, workspace).await?;
    Ok(())
}

async fn delete_network_volume<W>(
    workspace: &mut crate::domain::workspace::Workspace,
    workspace_catalog: &W,
    runpod_client: &dyn RunpodRuntimeClient,
    event_sink: &Arc<dyn EventSink<RunpodRuntimeEvent>>,
) -> Result<(), RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
{
    let Some(volume_id) = runpod_resources(workspace).network_volume_id.clone() else {
        return Ok(());
    };
    runpod_client.delete_network_volume(&volume_id).await?;
    runpod_resources_mut(workspace).network_volume_id = None;
    *workspace = persist_workspace(workspace_catalog, event_sink, workspace).await?;
    Ok(())
}

fn runpod_resources(
    workspace: &crate::domain::workspace::Workspace,
) -> &crate::domain::runpod::RunpodResources {
    let crate::domain::workspace::WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
    &runtime.resources
}

fn runpod_resources_mut(
    workspace: &mut crate::domain::workspace::Workspace,
) -> &mut crate::domain::runpod::RunpodResources {
    let crate::domain::workspace::WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
    &mut runtime.resources
}
