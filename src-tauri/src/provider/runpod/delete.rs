use crate::{
    domain::lifecycle_operation::LifecycleOperation,
    domain::workspace::Workspace,
    workspace::{WorkspaceError, WorkspaceRuntimeContext},
};

use super::client::RunpodRuntimeClient;

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
