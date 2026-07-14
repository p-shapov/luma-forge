use secrecy::SecretString;

use crate::application::{
    runtimes::{
        runpod::{ProvisionRunpodRuntime, RunpodRuntimeService},
        ProvisionRuntime, RuntimeError, RuntimeOperationQueryService, RuntimeService,
    },
    secrets::{SecretKind, SecretsService},
    workspace::WorkspaceService,
};

use super::{errors::*, model::*};

pub struct FacadeState {
    workspaces: WorkspaceService,
    secrets: SecretsService,
    operations: RuntimeOperationQueryService,
    runtimes: RuntimeService,
    runpod: RunpodRuntimeService,
}

impl FacadeState {
    pub fn new(
        workspaces: WorkspaceService,
        secrets: SecretsService,
        operations: RuntimeOperationQueryService,
        runtimes: RuntimeService,
        runpod: RunpodRuntimeService,
    ) -> Self {
        Self {
            workspaces,
            secrets,
            operations,
            runtimes,
            runpod,
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
            .start_provision(provision_command(&request.workspace_id, request.runtime))
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
        let (workspace, operation) = self.runtimes.start_cleanup(&request.workspace_id).await?;
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
        Ok(self.runpod.placement().await?.into())
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
        self.runtimes.recover_interrupted().await
    }
}

fn provision_command(workspace_id: &str, input: ProvisionRuntimeInput) -> ProvisionRuntime {
    match input {
        ProvisionRuntimeInput::Runpod {
            datacenter_id,
            gpu_id,
            volume_size_gb,
        } => ProvisionRuntime::Runpod(ProvisionRunpodRuntime {
            workspace_id: workspace_id.to_owned(),
            datacenter_id,
            gpu_id,
            volume_size_gb,
        }),
    }
}

#[cfg(test)]
mod tests {
    use crate::application::runtimes::ProvisionRuntime;

    use super::{provision_command, ProvisionRuntimeInput};

    #[test]
    fn provision_input_maps_to_the_application_command() {
        let ProvisionRuntime::Runpod(command) = provision_command(
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
}
