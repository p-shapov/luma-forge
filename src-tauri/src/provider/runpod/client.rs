use crate::runpod_runtime::provider::RunpodRuntimeClient as LegacyRunpodRuntimeClient;
use crate::{
    runpod_runtime::{errors::RunpodRuntimeError, provider as legacy},
    secrets::{ApiKeyIdentityProvider, SecretStore, SecretsService},
    shared::AppFuture,
    workspace::WorkspaceError,
};

pub use legacy::{
    CreateRunpodNetworkVolumeParams, CreateRunpodServerlessEndpointParams,
    CreateRunpodServerlessTemplateParams, RunpodProvisionerStatus, StartRunpodProvisionerPodParams,
};

pub trait RunpodRuntimeClient: Send + Sync {
    fn placement_options<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<crate::domain::runpod::RunpodPlacementOptions, WorkspaceError>>;

    fn create_network_volume<'a>(
        &'a self,
        params: CreateRunpodNetworkVolumeParams,
    ) -> AppFuture<'a, Result<String, WorkspaceError>>;

    fn delete_network_volume<'a>(
        &'a self,
        network_volume_id: &'a str,
    ) -> AppFuture<'a, Result<(), WorkspaceError>>;

    fn start_provisioner_pod<'a>(
        &'a self,
        params: StartRunpodProvisionerPodParams,
    ) -> AppFuture<'a, Result<String, WorkspaceError>>;

    fn terminate_provisioner_pod<'a>(
        &'a self,
        provisioner_pod_id: &'a str,
    ) -> AppFuture<'a, Result<(), WorkspaceError>>;

    fn get_provisioner_status<'a>(
        &'a self,
        workspace_id: &'a str,
        provisioner_pod_id: &'a str,
    ) -> AppFuture<'a, Result<RunpodProvisionerStatus, WorkspaceError>>;

    fn create_serverless_template<'a>(
        &'a self,
        params: CreateRunpodServerlessTemplateParams,
    ) -> AppFuture<'a, Result<String, WorkspaceError>>;

    fn create_serverless_endpoint<'a>(
        &'a self,
        params: CreateRunpodServerlessEndpointParams,
    ) -> AppFuture<'a, Result<String, WorkspaceError>>;

    fn delete_serverless_endpoint<'a>(
        &'a self,
        endpoint_id: &'a str,
    ) -> AppFuture<'a, Result<(), WorkspaceError>>;

    fn delete_template<'a>(
        &'a self,
        template_id: &'a str,
    ) -> AppFuture<'a, Result<(), WorkspaceError>>;
}

pub struct RunpodRuntimeProvider<RS, RI, HS, HI> {
    inner: legacy::RunpodRuntimeProvider<RS, RI, HS, HI>,
}

impl<RS, RI, HS, HI> RunpodRuntimeProvider<RS, RI, HS, HI>
where
    RS: SecretStore + 'static,
    RI: ApiKeyIdentityProvider + 'static,
    HS: SecretStore + 'static,
    HI: ApiKeyIdentityProvider + 'static,
{
    pub fn new(
        runpod_secrets: SecretsService<RS, RI>,
        hugging_face_secrets: SecretsService<HS, HI>,
    ) -> Self {
        Self {
            inner: legacy::RunpodRuntimeProvider::new(runpod_secrets, hugging_face_secrets),
        }
    }
}

impl<RS, RI, HS, HI> RunpodRuntimeClient for RunpodRuntimeProvider<RS, RI, HS, HI>
where
    RS: SecretStore,
    RI: ApiKeyIdentityProvider,
    HS: SecretStore,
    HI: ApiKeyIdentityProvider,
{
    fn placement_options<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<crate::domain::runpod::RunpodPlacementOptions, WorkspaceError>> {
        Box::pin(async move {
            self.inner
                .placement_options()
                .await
                .map_err(map_runpod_error)
        })
    }

    fn create_network_volume<'a>(
        &'a self,
        params: CreateRunpodNetworkVolumeParams,
    ) -> AppFuture<'a, Result<String, WorkspaceError>> {
        Box::pin(async move {
            self.inner
                .create_network_volume(params)
                .await
                .map_err(map_runpod_error)
        })
    }

    fn delete_network_volume<'a>(
        &'a self,
        network_volume_id: &'a str,
    ) -> AppFuture<'a, Result<(), WorkspaceError>> {
        Box::pin(async move {
            self.inner
                .delete_network_volume(network_volume_id)
                .await
                .map_err(map_runpod_error)
        })
    }

    fn start_provisioner_pod<'a>(
        &'a self,
        params: StartRunpodProvisionerPodParams,
    ) -> AppFuture<'a, Result<String, WorkspaceError>> {
        Box::pin(async move {
            self.inner
                .start_provisioner_pod(params)
                .await
                .map_err(map_runpod_error)
        })
    }

    fn terminate_provisioner_pod<'a>(
        &'a self,
        provisioner_pod_id: &'a str,
    ) -> AppFuture<'a, Result<(), WorkspaceError>> {
        Box::pin(async move {
            self.inner
                .terminate_provisioner_pod(provisioner_pod_id)
                .await
                .map_err(map_runpod_error)
        })
    }

    fn get_provisioner_status<'a>(
        &'a self,
        workspace_id: &'a str,
        provisioner_pod_id: &'a str,
    ) -> AppFuture<'a, Result<RunpodProvisionerStatus, WorkspaceError>> {
        Box::pin(async move {
            self.inner
                .get_provisioner_status(workspace_id, provisioner_pod_id)
                .await
                .map_err(map_runpod_error)
        })
    }

    fn create_serverless_template<'a>(
        &'a self,
        params: CreateRunpodServerlessTemplateParams,
    ) -> AppFuture<'a, Result<String, WorkspaceError>> {
        Box::pin(async move {
            self.inner
                .create_serverless_template(params)
                .await
                .map_err(map_runpod_error)
        })
    }

    fn create_serverless_endpoint<'a>(
        &'a self,
        params: CreateRunpodServerlessEndpointParams,
    ) -> AppFuture<'a, Result<String, WorkspaceError>> {
        Box::pin(async move {
            self.inner
                .create_serverless_endpoint(params)
                .await
                .map_err(map_runpod_error)
        })
    }

    fn delete_serverless_endpoint<'a>(
        &'a self,
        endpoint_id: &'a str,
    ) -> AppFuture<'a, Result<(), WorkspaceError>> {
        Box::pin(async move {
            self.inner
                .delete_serverless_endpoint(endpoint_id)
                .await
                .map_err(map_runpod_error)
        })
    }

    fn delete_template<'a>(
        &'a self,
        template_id: &'a str,
    ) -> AppFuture<'a, Result<(), WorkspaceError>> {
        Box::pin(async move {
            self.inner
                .delete_template(template_id)
                .await
                .map_err(map_runpod_error)
        })
    }
}

fn map_runpod_error(error: RunpodRuntimeError) -> WorkspaceError {
    match error {
        RunpodRuntimeError::RunpodApiError(error) => WorkspaceError::ProviderApiError(error),
        RunpodRuntimeError::RunpodApiKeyUnavailable(error) => {
            WorkspaceError::RuntimeProviderApiKeyUnavailable(error)
        }
        RunpodRuntimeError::HuggingFaceApiKeyUnavailable(error) => {
            WorkspaceError::WorkflowProviderApiKeyUnavailable(error)
        }
        RunpodRuntimeError::WorkflowCatalogInvalid(error) => {
            WorkspaceError::WorkflowCatalogInvalid(error)
        }
        RunpodRuntimeError::RuntimeCatalogInvalid(error) => {
            WorkspaceError::RuntimeCatalogInvalid(error)
        }
        RunpodRuntimeError::WorkspaceCatalogInvalid(error) => {
            WorkspaceError::WorkspaceCatalogInvalid(error)
        }
        RunpodRuntimeError::ProvisionerWorkerUnavailable { message } => {
            WorkspaceError::ProvisionerWorkerUnavailable { message }
        }
        RunpodRuntimeError::ProvisionerWorkerResponseInvalid { message } => {
            WorkspaceError::ProvisionerWorkerResponseInvalid { message }
        }
        RunpodRuntimeError::ProvisionerWorkerFailed { message } => {
            WorkspaceError::ProvisionerWorkerFailed { message }
        }
        RunpodRuntimeError::WorkspaceNotFound { workspace_id } => {
            WorkspaceError::WorkspaceNotFound { workspace_id }
        }
        RunpodRuntimeError::LifecycleOperationAlreadyRunning { workspace_id } => {
            WorkspaceError::LifecycleOperationAlreadyRunning { workspace_id }
        }
        RunpodRuntimeError::InvalidRuntimeState { message } => {
            WorkspaceError::InvalidState { message }
        }
    }
}
