use tauri::{AppHandle, Manager};

use crate::{
    application::provider_setup::ProviderSetupService,
    infrastructure::{
        database,
        provider_setup_repository::SqliteProviderSetupRepository,
        providers::{GpuProviderRegistry, RunPodProvider},
        secrets::{KeyringProviderApiKeyStore, KEYRING_SERVICE},
    },
};

pub(crate) struct AppState {
    provider_setup_service: ProviderSetupService,
}

impl AppState {
    pub(crate) async fn new(app: AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let app_data_dir = app.path().app_data_dir()?;
        std::fs::create_dir_all(&app_data_dir)?;
        let db_path = app_data_dir.join("luma-forge.sqlite3");
        let pool = database::connect(&db_path).await?;

        Ok(Self::from_pool(pool))
    }

    fn from_pool(pool: sqlx::SqlitePool) -> Self {
        let repository = SqliteProviderSetupRepository::new(pool);
        let key_store = KeyringProviderApiKeyStore::new(KEYRING_SERVICE);
        let registry = GpuProviderRegistry::new(RunPodProvider::default());
        let provider_setup_service = ProviderSetupService::new(repository, key_store, registry);

        Self {
            provider_setup_service,
        }
    }

    pub(crate) fn provider_setup_service(&self) -> ProviderSetupService {
        self.provider_setup_service.clone()
    }
}
