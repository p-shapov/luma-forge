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

use super::runtime::RunpodRuntimeClient;

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

#[cfg(test)]
mod tests {
    use crate::{
        domain::{
            lifecycle_operation::LifecycleOperationPayload,
            workspace::{WorkspaceRuntime, WorkspaceState},
        },
        provider::errors::ProviderApiError,
        provider::runpod::test_support::{
            runpod_client_with_failure, runpod_client_with_state, workspace_with_runpod_resources,
            RunpodClientFailure,
        },
        workspace::test_support::runtime_context_for_test,
        workspace::WorkspaceError,
    };

    #[tokio::test]
    async fn cleanup_persists_cleanup_payload() {
        let context = runtime_context_for_test();
        let workspace = workspace_with_runpod_resources("workspace-1");
        context.insert_workspace_for_test(workspace.clone()).await;
        let operation = context.create_operation_for_test("workspace-1").await;
        let runpod_client = runpod_client_with_state();

        let cleaned = super::cleanup_workspace(
            context.clone(),
            runpod_client.as_ref(),
            operation,
            workspace,
        )
        .await
        .expect("cleanup");

        assert_eq!(cleaned.state, WorkspaceState::NotProvisioned);
        let WorkspaceRuntime::Runpod(runtime) = cleaned.runtime;
        assert_eq!(runtime.resources.endpoint_id, None);
        assert_eq!(runtime.resources.template_id, None);
        assert_eq!(runtime.resources.provisioner_pod_id, None);
        assert_eq!(runtime.resources.network_volume_id, None);

        let persisted = context
            .find_workspace_for_test("workspace-1")
            .await
            .expect("persisted workspace");
        let WorkspaceRuntime::Runpod(runtime) = persisted.runtime;
        assert_eq!(runtime.resources.endpoint_id, None);
        assert_eq!(runtime.resources.template_id, None);
        assert_eq!(runtime.resources.provisioner_pod_id, None);
        assert_eq!(runtime.resources.network_volume_id, None);

        let latest = context
            .latest_operation_for_test("workspace-1")
            .await
            .expect("latest operation");
        assert!(matches!(
            latest.payload,
            Some(LifecycleOperationPayload::Cleanup(_))
        ));
    }

    #[tokio::test]
    async fn cleanup_stops_at_failed_remote_delete_step() {
        let cases = [
            (
                RunpodClientFailure::DeleteEndpoint,
                Some("endpoint"),
                Some("template"),
                Some("provisioner"),
                Some("volume"),
            ),
            (
                RunpodClientFailure::DeleteTemplate,
                None,
                Some("template"),
                Some("provisioner"),
                Some("volume"),
            ),
            (
                RunpodClientFailure::TerminateProvisionerPod,
                None,
                None,
                Some("provisioner"),
                Some("volume"),
            ),
            (
                RunpodClientFailure::DeleteNetworkVolume,
                None,
                None,
                None,
                Some("volume"),
            ),
        ];

        for (
            failure,
            expected_endpoint_id,
            expected_template_id,
            expected_provisioner_pod_id,
            expected_network_volume_id,
        ) in cases
        {
            let workspace_id = format!("workspace-{failure:?}");
            let context = runtime_context_for_test();
            let workspace = workspace_with_runpod_resources(&workspace_id);
            context.insert_workspace_for_test(workspace.clone()).await;
            let operation = context.create_operation_for_test(&workspace_id).await;
            let runpod_client = runpod_client_with_failure(failure);

            let error = super::cleanup_workspace(
                context.clone(),
                runpod_client.as_ref(),
                operation,
                workspace,
            )
            .await
            .expect_err("cleanup should fail");

            assert_eq!(
                error,
                WorkspaceError::ProviderApiError(ProviderApiError::RequestFailed {
                    message: failure.message().to_string(),
                })
            );
            let persisted = context
                .find_workspace_for_test(&workspace_id)
                .await
                .expect("persisted workspace");
            let WorkspaceRuntime::Runpod(runtime) = persisted.runtime;
            assert_eq!(
                runtime.resources.endpoint_id,
                expected_endpoint_id.map(str::to_string)
            );
            assert_eq!(
                runtime.resources.template_id,
                expected_template_id.map(str::to_string)
            );
            assert_eq!(
                runtime.resources.provisioner_pod_id,
                expected_provisioner_pod_id.map(str::to_string)
            );
            assert_eq!(
                runtime.resources.network_volume_id,
                expected_network_volume_id.map(str::to_string)
            );
        }
    }
}
