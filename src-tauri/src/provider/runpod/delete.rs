use crate::{
    domain::lifecycle_operation::LifecycleOperation,
    domain::workspace::Workspace,
    workspace::{WorkspaceError, WorkspaceRuntimeContext},
};

use super::runtime::RunpodRuntimeClient;

pub async fn delete_workspace(
    context: WorkspaceRuntimeContext<'_>,
    runpod_client: &dyn RunpodRuntimeClient,
    mut operation: LifecycleOperation,
    mut workspace: Workspace,
) -> Result<Workspace, WorkspaceError> {
    super::cleanup::cleanup_remote_resources(
        &context,
        Some(&mut operation),
        runpod_client,
        &mut workspace,
    )
    .await?;
    context.delete_workspace(&workspace.id).await?;
    Ok(workspace)
}

#[cfg(test)]
mod tests {
    use crate::{
        domain::lifecycle_operation::LifecycleOperationPayload,
        provider::runpod::test_support::{
            runpod_client_with_failure, runpod_client_with_state, workspace_with_runpod_resources,
            RunpodClientFailure,
        },
        shared::ApiError,
        workspace::test_support::runtime_context_for_test,
        workspace::WorkspaceError,
    };

    #[tokio::test]
    async fn delete_removes_workspace_without_persisting_delete_payload() {
        let context = runtime_context_for_test();
        let workspace = workspace_with_runpod_resources("workspace-1");
        context.insert_workspace_for_test(workspace.clone()).await;
        let operation = context.create_operation_for_test("workspace-1").await;
        let runpod_client = runpod_client_with_state();

        super::delete_workspace(
            context.clone(),
            runpod_client.as_ref(),
            operation,
            workspace,
        )
        .await
        .expect("delete");

        assert!(context
            .find_workspace_for_test("workspace-1")
            .await
            .is_none());
        assert!(matches!(
            context
                .latest_operation_for_test("workspace-1")
                .await
                .expect("latest operation")
                .payload,
            Some(LifecycleOperationPayload::Cleanup(_))
        ));
    }

    #[tokio::test]
    async fn delete_keeps_workspace_when_remote_cleanup_step_fails() {
        for failure in [
            RunpodClientFailure::DeleteEndpoint,
            RunpodClientFailure::DeleteTemplate,
            RunpodClientFailure::TerminateProvisionerPod,
            RunpodClientFailure::DeleteNetworkVolume,
        ] {
            let workspace_id = format!("workspace-{failure:?}");
            let context = runtime_context_for_test();
            let workspace = workspace_with_runpod_resources(&workspace_id);
            context.insert_workspace_for_test(workspace.clone()).await;
            let operation = context.create_operation_for_test(&workspace_id).await;
            let runpod_client = runpod_client_with_failure(failure);

            let error = super::delete_workspace(
                context.clone(),
                runpod_client.as_ref(),
                operation,
                workspace,
            )
            .await
            .expect_err("delete should fail");

            assert_eq!(
                error,
                WorkspaceError::ProviderApiError(ApiError::RequestFailed {
                    message: failure.message().to_string(),
                })
            );
            assert!(context
                .find_workspace_for_test(&workspace_id)
                .await
                .is_some());
        }
    }
}
