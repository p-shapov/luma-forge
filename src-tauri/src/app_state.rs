use std::path::PathBuf;

use tauri::{AppHandle, Manager};
use tokio::sync::OnceCell;

use crate::{
    bundled_catalog::reader::BundledCatalogReader,
    hugging_face_setup::{HuggingFaceSetupError, HuggingFaceSetupService},
    provider::{
        huggingface::{HuggingFaceClient, HuggingFaceHttpClientInitError},
        runpod::RunPodHttpClientInitError,
    },
    provider_setup::{
        ProviderSetupCoordinator, ProviderSetupProviderRegistry, ProviderSetupService,
    },
    secrets::{BlockingSecretStore, KeyringSecretStore},
    workspace_catalog::{repository::UnavailableWorkspaceCatalog, sqlite::SqliteWorkspaceCatalog},
    workspace_provisioning::{
        ProvisionerWorkerHttpGateway, ProvisionerWorkerHttpGatewayInitError,
        WorkspaceProvisioningConfig, WorkspaceProvisioningCoordinator, WorkspaceProvisioningError,
        WorkspaceProvisioningService,
    },
    workspace_removal::{WorkspaceRemovalError, WorkspaceRemovalService},
    workspace_resources::WorkspaceResourceProviderRegistry,
    workspace_resources::WorkspaceResourceService,
    workspace_setup::{
        error::WorkspaceSetupError, WorkspaceSetupProviderRegistry, WorkspaceSetupService,
    },
};

type ProductionSecretStore = BlockingSecretStore<KeyringSecretStore>;

pub(crate) type ProductionProviderSetupService = ProviderSetupService<ProductionSecretStore>;
pub(crate) type ProductionHuggingFaceSetupService =
    HuggingFaceSetupService<ProductionSecretStore, HuggingFaceClient>;
pub(crate) type WorkspaceSetupReadService =
    WorkspaceSetupService<BundledCatalogReader, ProductionSecretStore, UnavailableWorkspaceCatalog>;
pub(crate) type WorkspaceSetupWriteService =
    WorkspaceSetupService<BundledCatalogReader, ProductionSecretStore, SqliteWorkspaceCatalog>;
pub(crate) type ProductionWorkspaceProvisioningService = WorkspaceProvisioningService<
    ProductionSecretStore,
    SqliteWorkspaceCatalog,
    ProvisionerWorkerHttpGateway,
>;
pub(crate) type ProductionWorkspaceRemovalService = WorkspaceRemovalService<
    SqliteWorkspaceCatalog,
    WorkspaceResourceService<ProductionSecretStore, SqliteWorkspaceCatalog>,
>;

pub(crate) struct NativeAppState {
    workspace_catalog_source: WorkspaceCatalogSource,
    workspace_catalog: OnceCell<SqliteWorkspaceCatalog>,
    provider_setup_coordinator: ProviderSetupCoordinator,
    workspace_provisioning_coordinator: WorkspaceProvisioningCoordinator,
    catalogs: BundledCatalogReader,
    secrets: ProductionSecretStore,
    hugging_face_identity: Result<HuggingFaceClient, HuggingFaceHttpClientInitError>,
    provider_setup_registry: Result<ProviderSetupProviderRegistry, RunPodHttpClientInitError>,
    workspace_setup_registry: Result<WorkspaceSetupProviderRegistry, RunPodHttpClientInitError>,
    workspace_resource_registry:
        Result<WorkspaceResourceProviderRegistry, RunPodHttpClientInitError>,
    provisioner_workers:
        Result<ProvisionerWorkerHttpGateway, ProvisionerWorkerHttpGatewayInitError>,
}

impl NativeAppState {
    pub(crate) fn new(app: AppHandle) -> Self {
        let app_identifier = app.config().identifier.clone();
        Self::from_workspace_catalog_source(WorkspaceCatalogSource::AppDataDir(app), app_identifier)
    }

    pub(crate) fn provider_setup_service(
        &self,
    ) -> Result<ProductionProviderSetupService, crate::provider_setup::ProviderSetupError> {
        Ok(ProviderSetupService::with_provider_registry(
            self.secrets.clone(),
            self.provider_setup_registry
                .clone()
                .map_err(|_| crate::provider_setup::ProviderSetupError::ProviderApiUnavailable)?,
        ))
    }

    pub(crate) fn hugging_face_setup_service(
        &self,
    ) -> Result<ProductionHuggingFaceSetupService, HuggingFaceSetupError> {
        Ok(HuggingFaceSetupService::new(
            self.secrets.clone(),
            self.hugging_face_identity
                .clone()
                .map_err(|_| HuggingFaceSetupError::HuggingFaceApiUnavailable)?,
        ))
    }

    pub(crate) fn workspace_setup_read_service(
        &self,
    ) -> Result<WorkspaceSetupReadService, WorkspaceSetupError> {
        Ok(WorkspaceSetupService::with_provider_registry(
            self.catalogs.clone(),
            self.secrets.clone(),
            UnavailableWorkspaceCatalog,
            self.workspace_setup_registry
                .clone()
                .map_err(|_| WorkspaceSetupError::ProviderApiUnavailable)?,
        ))
    }

    pub(crate) async fn workspace_setup_service(
        &self,
    ) -> Result<WorkspaceSetupWriteService, WorkspaceSetupError> {
        let workspace_catalog = self.workspace_catalog().await?;
        Ok(WorkspaceSetupService::with_provider_registry(
            self.catalogs.clone(),
            self.secrets.clone(),
            workspace_catalog,
            self.workspace_setup_registry
                .clone()
                .map_err(|_| WorkspaceSetupError::ProviderApiUnavailable)?,
        ))
    }

    pub(crate) async fn workspace_provisioning_service(
        &self,
    ) -> Result<ProductionWorkspaceProvisioningService, WorkspaceProvisioningError> {
        let workspace_catalog = self
            .workspace_catalog()
            .await
            .map_err(crate::workspace_provisioning::helpers::catalog_error)?;
        let resources = WorkspaceResourceService::with_provider_registry(
            self.secrets.clone(),
            workspace_catalog.clone(),
            self.workspace_resource_registry
                .clone()
                .map_err(crate::workspace_resources::WorkspaceResourceError::from)?,
        );
        Ok(WorkspaceProvisioningService::new(
            self.secrets.clone(),
            resources,
            workspace_catalog,
            self.provisioner_workers.clone()?,
            self.workspace_provisioning_coordinator.clone(),
            WorkspaceProvisioningConfig,
        ))
    }

    pub(crate) async fn workspace_removal_service(
        &self,
    ) -> Result<ProductionWorkspaceRemovalService, WorkspaceRemovalError> {
        let workspace_catalog = self
            .workspace_catalog()
            .await
            .map_err(WorkspaceRemovalError::from)?;
        let resources = WorkspaceResourceService::with_provider_registry(
            self.secrets.clone(),
            workspace_catalog.clone(),
            self.workspace_resource_registry
                .clone()
                .map_err(crate::workspace_resources::WorkspaceResourceError::from)?,
        );
        Ok(WorkspaceRemovalService::new(
            workspace_catalog,
            resources,
            self.workspace_provisioning_coordinator.clone(),
        ))
    }

    pub(crate) fn provider_setup_coordinator(&self) -> &ProviderSetupCoordinator {
        &self.provider_setup_coordinator
    }

    async fn workspace_catalog(&self) -> Result<SqliteWorkspaceCatalog, WorkspaceSetupError> {
        self.workspace_catalog
            .get_or_try_init(|| async {
                SqliteWorkspaceCatalog::connect(self.workspace_catalog_source.catalog_path()?).await
            })
            .await
            .cloned()
    }

    fn from_workspace_catalog_source(
        workspace_catalog_source: WorkspaceCatalogSource,
        app_identifier: impl AsRef<str>,
    ) -> Self {
        Self::from_initialized_parts(
            workspace_catalog_source,
            app_identifier,
            HuggingFaceClient::try_new_default(),
            ProviderSetupProviderRegistry::try_new(),
            WorkspaceSetupProviderRegistry::try_new(),
            WorkspaceResourceProviderRegistry::try_new(),
            ProvisionerWorkerHttpGateway::try_new(),
        )
    }

    fn from_initialized_parts(
        workspace_catalog_source: WorkspaceCatalogSource,
        app_identifier: impl AsRef<str>,
        hugging_face_identity: Result<HuggingFaceClient, HuggingFaceHttpClientInitError>,
        provider_setup_registry: Result<ProviderSetupProviderRegistry, RunPodHttpClientInitError>,
        workspace_setup_registry: Result<WorkspaceSetupProviderRegistry, RunPodHttpClientInitError>,
        workspace_resource_registry: Result<
            WorkspaceResourceProviderRegistry,
            RunPodHttpClientInitError,
        >,
        provisioner_workers: Result<
            ProvisionerWorkerHttpGateway,
            ProvisionerWorkerHttpGatewayInitError,
        >,
    ) -> Self {
        let secrets = BlockingSecretStore::new(KeyringSecretStore::new(app_identifier));

        Self {
            workspace_catalog_source,
            workspace_catalog: OnceCell::new(),
            provider_setup_coordinator: ProviderSetupCoordinator::default(),
            workspace_provisioning_coordinator: WorkspaceProvisioningCoordinator::default(),
            catalogs: BundledCatalogReader,
            secrets,
            hugging_face_identity,
            provider_setup_registry,
            workspace_setup_registry,
            workspace_resource_registry,
            provisioner_workers,
        }
    }
}

enum WorkspaceCatalogSource {
    AppDataDir(AppHandle),
    #[cfg(test)]
    Test(PathBuf),
}

impl WorkspaceCatalogSource {
    fn catalog_path(&self) -> Result<PathBuf, WorkspaceSetupError> {
        match self {
            Self::AppDataDir(app) => app
                .path()
                .app_data_dir()
                .map(|data_dir| data_dir.join("workspace-catalog.sqlite"))
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogStorageUnavailable),
            #[cfg(test)]
            Self::Test(path) => Ok(path.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::huggingface::HuggingFaceHttpClientInitError;
    use crate::provider::runpod::RunPodHttpClientInitError;
    use crate::workspace_provisioning::ProvisionerWorkerHttpGatewayInitError;

    #[test]
    fn app_state_creation_preserves_http_init_errors_for_command_services() {
        let state = NativeAppState::from_initialized_parts(
            WorkspaceCatalogSource::Test(PathBuf::from("unused.sqlite")),
            "test.bundle",
            Err(HuggingFaceHttpClientInitError),
            Err(RunPodHttpClientInitError),
            Err(RunPodHttpClientInitError),
            Err(RunPodHttpClientInitError),
            Err(ProvisionerWorkerHttpGatewayInitError),
        );

        assert!(matches!(
            state.hugging_face_setup_service(),
            Err(HuggingFaceSetupError::HuggingFaceApiUnavailable)
        ));
        assert!(matches!(
            state.provider_setup_service(),
            Err(crate::provider_setup::ProviderSetupError::ProviderApiUnavailable)
        ));
        assert!(matches!(
            state.workspace_setup_read_service(),
            Err(WorkspaceSetupError::ProviderApiUnavailable)
        ));
    }

    #[tokio::test]
    async fn app_state_maps_provisioning_http_init_errors_after_startup() {
        let catalog_path = std::env::temp_dir().join(format!(
            "luma-forge-app-state-test-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let state = NativeAppState::from_initialized_parts(
            WorkspaceCatalogSource::Test(catalog_path),
            "test.bundle",
            HuggingFaceClient::try_new_default(),
            ProviderSetupProviderRegistry::try_new(),
            WorkspaceSetupProviderRegistry::try_new(),
            Err(RunPodHttpClientInitError),
            Err(ProvisionerWorkerHttpGatewayInitError),
        );

        let result = state.workspace_provisioning_service().await;
        let _ = std::fs::remove_file(state.workspace_catalog_source.catalog_path().unwrap());

        assert!(matches!(
            result,
            Err(WorkspaceProvisioningError::ProviderApiUnavailable)
        ));
    }
}
