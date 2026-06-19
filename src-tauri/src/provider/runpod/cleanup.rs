use crate::{
    domain::{
        lifecycle_operation::{
            LifecycleCleanupPayload, LifecycleOperation, LifecycleOperationPayload,
        },
        runpod::{RunpodCleanupStep, RunpodLifecycleCleanupPayload},
        workspace::{Workspace, WorkspaceRuntime, WorkspaceState},
    },
    workspace::{WorkspaceError, WorkspaceRuntimeContext},
};

use super::client::RunpodRuntimeClient;

fn cleanup_payload(step: RunpodCleanupStep) -> LifecycleOperationPayload {
    LifecycleOperationPayload::Cleanup(LifecycleCleanupPayload::Runpod(
        RunpodLifecycleCleanupPayload { step: Some(step) },
    ))
}

async fn mark_step(
    context: &WorkspaceRuntimeContext<'_>,
    operation: &mut LifecycleOperation,
    step: RunpodCleanupStep,
) -> Result<(), WorkspaceError> {
    operation.payload = Some(cleanup_payload(step));
    *operation = context.persist_operation(operation.clone()).await?;
    Ok(())
}

pub async fn cleanup_remote_resources(
    context: &WorkspaceRuntimeContext<'_>,
    mut operation: Option<&mut LifecycleOperation>,
    runpod_client: &dyn RunpodRuntimeClient,
    workspace: &mut Workspace,
) -> Result<(), WorkspaceError> {
    let resources = runpod_resources_mut(workspace);
    if let Some(endpoint_id) = resources.endpoint_id.clone() {
        if let Some(operation) = operation.as_mut() {
            mark_step(context, operation, RunpodCleanupStep::DeleteEndpoint).await?;
        }
        runpod_client
            .delete_serverless_endpoint(&endpoint_id)
            .await
            .map_err(super::runtime::map_provider_error)?;
        runpod_resources_mut(workspace).endpoint_id = None;
        *workspace = context.persist_workspace(workspace.clone()).await?;
    }

    if let Some(template_id) = runpod_resources(workspace).template_id.clone() {
        if let Some(operation) = operation.as_mut() {
            mark_step(context, operation, RunpodCleanupStep::DeleteTemplate).await?;
        }
        runpod_client
            .delete_template(&template_id)
            .await
            .map_err(super::runtime::map_provider_error)?;
        runpod_resources_mut(workspace).template_id = None;
        *workspace = context.persist_workspace(workspace.clone()).await?;
    }

    if let Some(provisioner_pod_id) = runpod_resources(workspace).provisioner_pod_id.clone() {
        if let Some(operation) = operation.as_mut() {
            mark_step(
                context,
                operation,
                RunpodCleanupStep::TerminateProvisionerPod,
            )
            .await?;
        }
        runpod_client
            .terminate_provisioner_pod(&provisioner_pod_id)
            .await
            .map_err(super::runtime::map_provider_error)?;
        runpod_resources_mut(workspace).provisioner_pod_id = None;
        *workspace = context.persist_workspace(workspace.clone()).await?;
    }

    if let Some(network_volume_id) = runpod_resources(workspace).network_volume_id.clone() {
        if let Some(operation) = operation.as_mut() {
            mark_step(context, operation, RunpodCleanupStep::DeleteNetworkVolume).await?;
        }
        runpod_client
            .delete_network_volume(&network_volume_id)
            .await
            .map_err(super::runtime::map_provider_error)?;
        runpod_resources_mut(workspace).network_volume_id = None;
        *workspace = context.persist_workspace(workspace.clone()).await?;
    }

    Ok(())
}

pub async fn cleanup_workspace(
    context: WorkspaceRuntimeContext<'_>,
    runpod_client: &dyn RunpodRuntimeClient,
    mut operation: LifecycleOperation,
    mut workspace: Workspace,
) -> Result<Workspace, WorkspaceError> {
    cleanup_remote_resources(
        &context,
        Some(&mut operation),
        runpod_client,
        &mut workspace,
    )
    .await?;
    workspace.state = WorkspaceState::NotProvisioned;
    context.persist_workspace(workspace).await
}

fn runpod_resources(workspace: &Workspace) -> &crate::domain::runpod::RunpodResources {
    let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
    &runtime.resources
}

fn runpod_resources_mut(workspace: &mut Workspace) -> &mut crate::domain::runpod::RunpodResources {
    let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
    &mut runtime.resources
}
