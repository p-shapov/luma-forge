use std::path::PathBuf;

use tauri::{AppHandle, Manager};
use tokio::sync::OnceCell;

use crate::{
    app_config::NativeAppConfig,
    bundled_catalog::reader::BundledCatalogReader,
    provider::{runpod::RunPodClient, ProviderClientRegistry},
    provider_setup::{ProviderSetupCoordinator, ProviderSetupService},
    provisioner_worker::ProvisionerWorkerHttpGateway,
    secrets::KeyringSecretStore,
    workspace_catalog::{repository::UnavailableWorkspaceCatalog, sqlite::SqliteWorkspaceCatalog},
    workspace_provisioning::{
        WorkspaceProvisioningConfig, WorkspaceProvisioningCoordinator, WorkspaceProvisioningError,
        WorkspaceProvisioningService,
    },
    workspace_setup::{error::WorkspaceSetupError, WorkspaceSetupService},
};

pub(crate) type ProductionProviderSetupService =
    ProviderSetupService<KeyringSecretStore, ProviderClientRegistry>;
pub(crate) type WorkspaceSetupReadService = WorkspaceSetupService<
    BundledCatalogReader,
    KeyringSecretStore,
    ProviderClientRegistry,
    UnavailableWorkspaceCatalog,
>;
pub(crate) type WorkspaceSetupWriteService = WorkspaceSetupService<
    BundledCatalogReader,
    KeyringSecretStore,
    ProviderClientRegistry,
    SqliteWorkspaceCatalog,
>;
pub(crate) type ProductionWorkspaceProvisioningService = WorkspaceProvisioningService<
    KeyringSecretStore,
    ProviderClientRegistry,
    SqliteWorkspaceCatalog,
    ProvisionerWorkerHttpGateway,
>;

pub(crate) struct NativeAppState {
    workspace_catalog_source: WorkspaceCatalogSource,
    workspace_catalog: OnceCell<SqliteWorkspaceCatalog>,
    provider_setup_coordinator: ProviderSetupCoordinator,
    workspace_provisioning_coordinator: WorkspaceProvisioningCoordinator,
    app_config: NativeAppConfig,
    catalogs: BundledCatalogReader,
    secrets: KeyringSecretStore,
    providers: ProviderClientRegistry,
    provisioner_workers: ProvisionerWorkerHttpGateway,
    #[cfg(test)]
    workspace_catalog_initialization_count: std::sync::atomic::AtomicUsize,
}

impl NativeAppState {
    pub(crate) fn new(app: AppHandle) -> Self {
        let app_identifier = app.config().identifier.clone();
        Self::from_workspace_catalog_source(WorkspaceCatalogSource::AppDataDir(app), app_identifier)
    }

    pub(crate) fn provider_setup_service(&self) -> ProductionProviderSetupService {
        ProviderSetupService::new(self.secrets.clone(), self.providers.clone())
    }

    pub(crate) fn workspace_setup_read_service(&self) -> WorkspaceSetupReadService {
        WorkspaceSetupService::new(
            self.catalogs.clone(),
            self.secrets.clone(),
            self.providers.clone(),
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
            self.providers.clone(),
            workspace_catalog,
        ))
    }

    pub(crate) async fn workspace_provisioning_service(
        &self,
    ) -> Result<ProductionWorkspaceProvisioningService, WorkspaceProvisioningError> {
        let workspace_catalog = self
            .workspace_catalog()
            .await
            .map_err(|_| WorkspaceProvisioningError::WorkspaceCatalogUnavailable)?;
        Ok(WorkspaceProvisioningService::new(
            self.secrets.clone(),
            self.providers.clone(),
            workspace_catalog,
            self.provisioner_workers.clone(),
            self.workspace_provisioning_coordinator.clone(),
            WorkspaceProvisioningConfig {
                provisioner_worker_image_ref: self.app_config.provisioner_worker_image_ref.clone(),
                provisioner_worker_port: self.app_config.provisioner_worker_port,
                runpod_endpoint_worker_image_ref: self
                    .app_config
                    .runpod_endpoint_worker_image_ref
                    .clone(),
                runpod_endpoint_worker_port: self.app_config.runpod_endpoint_worker_port,
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
                #[cfg(test)]
                self.workspace_catalog_initialization_count
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

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
        let providers = ProviderClientRegistry::new(secrets.clone(), RunPodClient::default());

        Self {
            workspace_catalog_source,
            workspace_catalog: OnceCell::new(),
            provider_setup_coordinator: ProviderSetupCoordinator::default(),
            workspace_provisioning_coordinator: WorkspaceProvisioningCoordinator::default(),
            app_config: NativeAppConfig::from_build_environment(),
            catalogs: BundledCatalogReader,
            secrets,
            providers,
            provisioner_workers: ProvisionerWorkerHttpGateway::default(),
            #[cfg(test)]
            workspace_catalog_initialization_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    #[cfg(test)]
    fn with_workspace_catalog_path(path: PathBuf) -> Self {
        Self::from_workspace_catalog_source(
            WorkspaceCatalogSource::CatalogPath(path),
            "test.luma-forge",
        )
    }

    #[cfg(test)]
    fn workspace_catalog_initialization_count(&self) -> usize {
        self.workspace_catalog_initialization_count
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}

enum WorkspaceCatalogSource {
    AppDataDir(AppHandle),
    #[cfg(test)]
    CatalogPath(PathBuf),
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
            Self::CatalogPath(path) => Ok(path.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, time::Duration};

    use crate::{
        domain::provider_setup::GpuCloudProviderId, workspace_setup::error::WorkspaceSetupError,
    };

    use super::*;

    fn temp_catalog_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("luma-forge-{name}-{}.sqlite", uuid::Uuid::new_v4()))
    }

    #[tokio::test]
    async fn workspace_catalog_initialization_failure_uses_existing_error() {
        let base = std::env::temp_dir().join(format!(
            "luma-forge-blocked-catalog-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&base).expect("test directory should be created");
        let blocked_parent = base.join("not-a-directory");
        fs::write(&blocked_parent, "not a directory").expect("blocking file should be created");
        let state = NativeAppState::with_workspace_catalog_path(
            blocked_parent.join("workspace-catalog.sqlite"),
        );

        let error = match state.workspace_setup_service().await {
            Ok(_) => panic!("catalog initialization should fail"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            WorkspaceSetupError::WorkspaceCatalogStorageUnavailable
        );

        fs::remove_file(blocked_parent).ok();
        fs::remove_dir(base).ok();
    }

    #[tokio::test]
    async fn workspace_catalog_is_initialized_once_and_reused() {
        let path = temp_catalog_path("shared-catalog");
        let state = NativeAppState::with_workspace_catalog_path(path.clone());

        state
            .workspace_setup_service()
            .await
            .expect("first catalog initialization should succeed");
        state
            .workspace_setup_service()
            .await
            .expect("second catalog access should succeed");

        assert_eq!(state.workspace_catalog_initialization_count(), 1);

        fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn provider_setup_coordinator_is_shared_runtime_state() {
        let state = NativeAppState::with_workspace_catalog_path(temp_catalog_path("coordinator"));
        let provider_id = GpuCloudProviderId::Runpod;
        let first_guard = state.provider_setup_coordinator().lock(&provider_id).await;

        let wait_result = tokio::time::timeout(
            Duration::from_millis(10),
            state.provider_setup_coordinator().lock(&provider_id),
        )
        .await;
        assert!(wait_result.is_err());

        drop(first_guard);
        state.provider_setup_coordinator().lock(&provider_id).await;
    }
}
