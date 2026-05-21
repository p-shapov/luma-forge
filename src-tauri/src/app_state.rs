use std::path::PathBuf;

use tauri::{AppHandle, Manager};
use tokio::sync::OnceCell;

use crate::{
    bundled_catalog::reader::BundledCatalogReader,
    provider_setup::{ProviderSetupCoordinator, ProviderSetupService},
    secrets::KeyringSecretStore,
    workspace_catalog::{repository::UnavailableWorkspaceCatalog, sqlite::SqliteWorkspaceCatalog},
    workspace_provisioner::ProvisionerWorkerHttpGateway,
    workspace_provisioning::{
        WorkspaceProvisioningConfig, WorkspaceProvisioningCoordinator, WorkspaceProvisioningError,
        WorkspaceProvisioningService,
    },
    workspace_resources::WorkspaceResourceService,
    workspace_setup::{error::WorkspaceSetupError, WorkspaceSetupService},
};

pub(crate) type ProductionProviderSetupService = ProviderSetupService<KeyringSecretStore>;
pub(crate) type WorkspaceSetupReadService =
    WorkspaceSetupService<BundledCatalogReader, KeyringSecretStore, UnavailableWorkspaceCatalog>;
pub(crate) type WorkspaceSetupWriteService =
    WorkspaceSetupService<BundledCatalogReader, KeyringSecretStore, SqliteWorkspaceCatalog>;
pub(crate) type ProductionWorkspaceProvisioningService = WorkspaceProvisioningService<
    KeyringSecretStore,
    SqliteWorkspaceCatalog,
    ProvisionerWorkerHttpGateway,
>;

pub(crate) struct NativeAppState {
    workspace_catalog_source: WorkspaceCatalogSource,
    workspace_catalog: OnceCell<SqliteWorkspaceCatalog>,
    provider_setup_coordinator: ProviderSetupCoordinator,
    workspace_provisioning_coordinator: WorkspaceProvisioningCoordinator,
    catalogs: BundledCatalogReader,
    secrets: KeyringSecretStore,
    provisioner_workers: ProvisionerWorkerHttpGateway,
}

impl NativeAppState {
    pub(crate) fn new(app: AppHandle) -> Self {
        let app_identifier = app.config().identifier.clone();
        Self::from_workspace_catalog_source(WorkspaceCatalogSource::AppDataDir(app), app_identifier)
    }

    pub(crate) fn provider_setup_service(&self) -> ProductionProviderSetupService {
        ProviderSetupService::new(self.secrets.clone())
    }

    pub(crate) fn workspace_setup_read_service(&self) -> WorkspaceSetupReadService {
        WorkspaceSetupService::new(
            self.catalogs.clone(),
            self.secrets.clone(),
            UnavailableWorkspaceCatalog,
        )
    }

    pub(crate) async fn workspace_setup_service(
        &self,
    ) -> Result<WorkspaceSetupWriteService, WorkspaceSetupError> {
        let workspace_catalog = self.workspace_catalog().await?;
        Ok(WorkspaceSetupService::new(
            self.catalogs.clone(),
            self.secrets.clone(),
            workspace_catalog,
        ))
    }

    pub(crate) async fn workspace_provisioning_service(
        &self,
    ) -> Result<ProductionWorkspaceProvisioningService, WorkspaceProvisioningError> {
        let workspace_catalog = self
            .workspace_catalog()
            .await
            .map_err(crate::workspace_provisioning::helpers::catalog_error)?;
        let resources =
            WorkspaceResourceService::new(self.secrets.clone(), workspace_catalog.clone());
        Ok(WorkspaceProvisioningService::new(
            self.secrets.clone(),
            resources,
            workspace_catalog,
            self.provisioner_workers.clone(),
            self.workspace_provisioning_coordinator.clone(),
            WorkspaceProvisioningConfig {
                volume_mount_path: "/workspace".to_string(),
            },
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
        let secrets = KeyringSecretStore::new(app_identifier);

        Self {
            workspace_catalog_source,
            workspace_catalog: OnceCell::new(),
            provider_setup_coordinator: ProviderSetupCoordinator::default(),
            workspace_provisioning_coordinator: WorkspaceProvisioningCoordinator::default(),
            catalogs: BundledCatalogReader,
            secrets,
            provisioner_workers: ProvisionerWorkerHttpGateway::default(),
        }
    }
}

enum WorkspaceCatalogSource {
    AppDataDir(AppHandle),
}

impl WorkspaceCatalogSource {
    fn catalog_path(&self) -> Result<PathBuf, WorkspaceSetupError> {
        match self {
            Self::AppDataDir(app) => app
                .path()
                .app_data_dir()
                .map(|data_dir| data_dir.join("workspace-catalog.sqlite"))
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogStorageUnavailable),
        }
    }
}
