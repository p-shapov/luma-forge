use secrecy::SecretString;

use crate::application::{
    runtimes::{
        runpod::{ProvisionRunpodRuntime, RunpodRuntimeService},
        RuntimeError, RuntimeKind, RuntimeOperation, RuntimeOperationQueryService,
    },
    secrets::{SecretKind, SecretsService},
    workspace::{Workspace, WorkspaceService},
};

use super::{errors::*, model::*};

#[derive(Clone)]
pub struct RuntimeDispatcher {
    runpod: RunpodRuntimeService,
}

impl RuntimeDispatcher {
    pub fn new(runpod: RunpodRuntimeService) -> Self {
        Self { runpod }
    }

    pub async fn provision(
        &self,
        workspace_id: String,
        input: ProvisionRuntimeInput,
    ) -> Result<(Workspace, RuntimeOperation), RuntimeError> {
        match input {
            input @ ProvisionRuntimeInput::Runpod { .. } => {
                self.runpod
                    .start_provision(runpod_provision_command(&workspace_id, input))
                    .await
            }
        }
    }

    pub async fn cleanup(
        &self,
        workspace: Workspace,
    ) -> Result<(Workspace, RuntimeOperation), RuntimeError> {
        match attached_runtime_kind(&workspace)? {
            RuntimeKind::Runpod => self.runpod.start_cleanup(workspace).await,
        }
    }

    pub async fn recover_interrupted(
        &self,
        operations: Vec<RuntimeOperation>,
    ) -> Result<(), RuntimeError> {
        let mut runpod = Vec::new();
        for operation in operations {
            match operation.runtime_kind {
                RuntimeKind::Runpod => runpod.push(operation),
            }
        }
        self.runpod.recover_interrupted(runpod).await
    }
}

pub struct FacadeState {
    workspaces: WorkspaceService,
    secrets: SecretsService,
    operations: RuntimeOperationQueryService,
    runtimes: RuntimeDispatcher,
}

impl FacadeState {
    pub fn new(
        workspaces: WorkspaceService,
        secrets: SecretsService,
        operations: RuntimeOperationQueryService,
        runtimes: RuntimeDispatcher,
    ) -> Self {
        Self {
            workspaces,
            secrets,
            operations,
            runtimes,
        }
    }

    pub async fn get_workflows(
        &self,
        request: PageRequest,
    ) -> CommandResult<WorkflowPageDto, GetWorkflowsErrorCode> {
        let (offset, limit) = validate_page(request)?;
        let (workflows, total) = self.workspaces.list_workflows(offset, limit).await?;
        Ok(WorkflowPageDto {
            workflows: workflows.into_iter().map(Into::into).collect(),
            total,
        })
    }

    pub async fn get_workspaces(
        &self,
        request: PageRequest,
    ) -> CommandResult<WorkspacePageDto, GetWorkspacesErrorCode> {
        let (offset, limit) = validate_page(request)?;
        let (workspaces, total) = self.workspaces.list(offset, limit).await?;
        Ok(WorkspacePageDto {
            workspaces: workspaces
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
            total,
        })
    }

    pub async fn create_workspace(
        &self,
        request: CreateWorkspaceRequest,
    ) -> CommandResult<WorkspaceDto, CreateWorkspaceErrorCode> {
        Ok(self
            .workspaces
            .create(request.workflow.into())
            .await?
            .try_into()?)
    }

    pub async fn delete_workspace(
        &self,
        request: WorkspaceIdRequest,
    ) -> CommandResult<(), DeleteWorkspaceErrorCode> {
        self.workspaces.delete(&request.workspace_id).await?;
        Ok(())
    }

    pub async fn provision_workspace(
        &self,
        request: ProvisionWorkspaceRequest,
    ) -> CommandResult<WorkspaceOperationDto, ProvisionWorkspaceErrorCode> {
        let (workspace, operation) = self
            .runtimes
            .provision(request.workspace_id, request.runtime)
            .await?;
        Ok(WorkspaceOperationDto {
            workspace: workspace.try_into()?,
            operation: operation.try_into()?,
        })
    }

    pub async fn cleanup_workspace(
        &self,
        request: WorkspaceIdRequest,
    ) -> CommandResult<WorkspaceOperationDto, CleanupWorkspaceErrorCode> {
        let workspace = self.workspaces.get(&request.workspace_id).await?;
        let (workspace, operation) = self.runtimes.cleanup(workspace).await?;
        Ok(WorkspaceOperationDto {
            workspace: workspace.try_into()?,
            operation: operation.try_into()?,
        })
    }

    pub async fn get_runtime_operations(
        &self,
        request: RuntimeOperationPageRequest,
    ) -> CommandResult<RuntimeOperationPageDto, GetRuntimeOperationsErrorCode> {
        let (offset, limit) = validate_operation_page(&request)?;
        let (operations, total) = self
            .operations
            .page(request.workspace_id.as_deref(), offset, limit)
            .await?;
        Ok(RuntimeOperationPageDto {
            operations: operations
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
            total,
        })
    }

    pub async fn get_runpod_placement(
        &self,
    ) -> CommandResult<RunpodPlacementDto, GetRunpodPlacementErrorCode> {
        Ok(self.runtimes.runpod.placement().await?.into())
    }

    pub async fn setup_runpod_api_key(
        &self,
        request: SetupApiKeyRequest,
    ) -> CommandResult<IdentityDto, SetupApiKeyErrorCode> {
        Ok(self
            .secrets
            .set(
                SecretKind::RunpodApiKey,
                SecretString::from(request.api_key),
            )
            .await?
            .into())
    }

    pub async fn setup_hugging_face_api_key(
        &self,
        request: SetupApiKeyRequest,
    ) -> CommandResult<IdentityDto, SetupApiKeyErrorCode> {
        Ok(self
            .secrets
            .set(
                SecretKind::HuggingFaceApiKey,
                SecretString::from(request.api_key),
            )
            .await?
            .into())
    }

    pub async fn get_runpod_identity(&self) -> CommandResult<IdentityDto, GetIdentityErrorCode> {
        Ok(self
            .secrets
            .identity(SecretKind::RunpodApiKey)
            .await?
            .into())
    }

    pub async fn get_hugging_face_identity(
        &self,
    ) -> CommandResult<IdentityDto, GetIdentityErrorCode> {
        Ok(self
            .secrets
            .identity(SecretKind::HuggingFaceApiKey)
            .await?
            .into())
    }

    pub async fn delete_runpod_api_key(&self) -> CommandResult<(), DeleteApiKeyErrorCode> {
        self.secrets.delete(SecretKind::RunpodApiKey).await?;
        Ok(())
    }

    pub async fn delete_hugging_face_api_key(&self) -> CommandResult<(), DeleteApiKeyErrorCode> {
        self.secrets.delete(SecretKind::HuggingFaceApiKey).await?;
        Ok(())
    }

    pub async fn recover_interrupted(&self) -> Result<(), RuntimeError> {
        let operations = self.operations.running().await?;
        self.runtimes.recover_interrupted(operations).await
    }
}

fn runpod_provision_command(
    workspace_id: &str,
    input: ProvisionRuntimeInput,
) -> ProvisionRunpodRuntime {
    match input {
        ProvisionRuntimeInput::Runpod {
            datacenter_id,
            gpu_id,
            volume_size_gb,
        } => ProvisionRunpodRuntime {
            workspace_id: workspace_id.to_owned(),
            datacenter_id,
            gpu_id,
            volume_size_gb,
        },
    }
}

fn attached_runtime_kind(workspace: &Workspace) -> Result<RuntimeKind, RuntimeError> {
    workspace
        .runtime
        .as_ref()
        .map(|runtime| runtime.provider.kind())
        .ok_or(RuntimeError::NotProvisioned)
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use crate::application::{
        runtimes::{
            runpod::{RunpodRuntime, RunpodRuntimeConfig, RunpodRuntimeResources},
            CatalogRef, Runtime, RuntimeError, RuntimeKind, RuntimeProvider, RuntimeState,
        },
        workspace::Workspace,
    };

    use super::{attached_runtime_kind, runpod_provision_command};
    use crate::facade::ProvisionRuntimeInput;

    fn workspace_with_runpod_runtime() -> Workspace {
        Workspace {
            id: "workspace-1".into(),
            workflow: CatalogRef::new("workflow-1", "1"),
            created_at: OffsetDateTime::UNIX_EPOCH,
            runtime: Some(Runtime {
                state: RuntimeState::Ready,
                provider: RuntimeProvider::Runpod(RunpodRuntime {
                    config: RunpodRuntimeConfig {
                        datacenter_id: "EU-RO-1".into(),
                        gpu_id: "gpu-1".into(),
                        volume_size_gb: 100,
                    },
                    resources: RunpodRuntimeResources::default(),
                }),
            }),
        }
    }

    #[test]
    fn provision_dispatch_maps_the_runpod_input() {
        let command = runpod_provision_command(
            "workspace-1",
            ProvisionRuntimeInput::Runpod {
                datacenter_id: "EU-RO-1".into(),
                gpu_id: "gpu-1".into(),
                volume_size_gb: 100,
            },
        );
        assert_eq!(command.workspace_id, "workspace-1");
        assert_eq!(command.datacenter_id, "EU-RO-1");
        assert_eq!(command.gpu_id, "gpu-1");
        assert_eq!(command.volume_size_gb, 100);
    }

    #[test]
    fn cleanup_dispatch_selects_the_attached_provider() {
        let workspace = workspace_with_runpod_runtime();
        assert_eq!(attached_runtime_kind(&workspace), Ok(RuntimeKind::Runpod));
    }

    #[test]
    fn cleanup_dispatch_rejects_an_unprovisioned_workspace() {
        let workspace = Workspace {
            runtime: None,
            ..workspace_with_runpod_runtime()
        };
        assert_eq!(
            attached_runtime_kind(&workspace),
            Err(RuntimeError::NotProvisioned)
        );
    }
}
