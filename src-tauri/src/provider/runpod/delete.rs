use crate::{
    domain::workspace::Workspace,
    workspace::{WorkspaceError, WorkspaceRuntimeContext},
};

use super::client::RunpodRuntimeClient;

pub async fn delete_workspace(
    context: WorkspaceRuntimeContext<'_>,
    runpod_client: &dyn RunpodRuntimeClient,
    mut workspace: Workspace,
) -> Result<(), WorkspaceError> {
    super::cleanup::cleanup_remote_resources(&context, None, runpod_client, &mut workspace).await?;
    context.delete_workspace(&workspace.id).await
}
