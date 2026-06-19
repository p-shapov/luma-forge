use std::sync::Arc;

use crate::{
    domain::{lifecycle_operation::LifecycleOperation, workspace::Workspace},
    shared::{ApiError, AppFuture},
    workspace::{WorkspaceError, WorkspaceRuntime, WorkspaceRuntimeContext},
};

use super::{client::RunpodRuntimeClient, errors::RunpodProviderError};

#[derive(Clone)]
pub struct RunpodWorkspaceRuntime {
    runpod_client: Arc<dyn RunpodRuntimeClient>,
}

impl RunpodWorkspaceRuntime {
    pub fn new(runpod_client: Arc<dyn RunpodRuntimeClient>) -> Self {
        Self { runpod_client }
    }
}

pub(super) fn map_provider_error(error: RunpodProviderError) -> WorkspaceError {
    match error {
        RunpodProviderError::ProviderApiError(error) => WorkspaceError::ProviderApiError(error),
        RunpodProviderError::RuntimeProviderApiKeyUnavailable(error) => {
            WorkspaceError::RuntimeProviderApiKeyUnavailable(error)
        }
        RunpodProviderError::WorkflowProviderApiKeyUnavailable(error) => {
            WorkspaceError::WorkflowProviderApiKeyUnavailable(error)
        }
        RunpodProviderError::ProvisionerWorkerUnavailable { message }
        | RunpodProviderError::ProvisionerWorkerResponseInvalid { message }
        | RunpodProviderError::ProvisionerWorkerFailed { message } => {
            WorkspaceError::ProviderApiError(ApiError::RequestFailed { message })
        }
    }
}

impl WorkspaceRuntime for RunpodWorkspaceRuntime {
    fn provision<'a>(
        &'a self,
        context: WorkspaceRuntimeContext<'a>,
        operation: LifecycleOperation,
        workspace: Workspace,
    ) -> AppFuture<'a, Result<Workspace, WorkspaceError>> {
        Box::pin(async move {
            super::provision::provision_workspace(
                context,
                self.runpod_client.as_ref(),
                operation,
                workspace,
            )
            .await
        })
    }

    fn cleanup<'a>(
        &'a self,
        context: WorkspaceRuntimeContext<'a>,
        operation: LifecycleOperation,
        workspace: Workspace,
    ) -> AppFuture<'a, Result<Workspace, WorkspaceError>> {
        Box::pin(async move {
            super::cleanup::cleanup_workspace(
                context,
                self.runpod_client.as_ref(),
                operation,
                workspace,
            )
            .await
        })
    }

    fn delete<'a>(
        &'a self,
        context: WorkspaceRuntimeContext<'a>,
        operation: LifecycleOperation,
        workspace: Workspace,
    ) -> AppFuture<'a, Result<Workspace, WorkspaceError>> {
        Box::pin(async move {
            super::delete::delete_workspace(
                context,
                self.runpod_client.as_ref(),
                operation,
                workspace,
            )
            .await
        })
    }
}
