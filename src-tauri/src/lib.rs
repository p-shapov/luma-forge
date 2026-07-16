pub mod adapters;
pub mod application;
pub mod diagnostics;
pub mod facade;
pub mod infra;
pub mod providers;

use std::sync::Arc;

use adapters::{
    bundled::BundledCatalogAdapter,
    hugging_face::HuggingFaceIdentityAdapter,
    keyring::KeyringSecretStore,
    runpod::{RunpodIdentityAdapter, RunpodRuntimeProviderAdapter},
    sqlite::{
        SqliteRuntimeOperationRepository, SqliteRuntimeTransitionRepository,
        SqliteWorkspaceRepository,
    },
};
use application::{
    runtimes::{
        runpod::{RunpodRuntimeService, RunpodRuntimeServiceDependencies},
        RuntimeService,
    },
    secrets::SecretsService,
    workspace::WorkspaceService,
};
use facade::{FacadeState, TauriEventSink};
use infra::sqlite::database::SqliteInfraDatabase;
use tauri::Manager;

const DB_FILE_NAME: &str = "db.sqlite";
const DIAGNOSTICS_FILE_NAME: &str = "diagnostics.log";
const BUNDLED_DIR_NAME: &str = "bundled";

#[derive(Debug, thiserror::Error)]
enum BootstrapError {
    #[error("app data directory is unavailable")]
    AppDataDirectoryUnavailable,
    #[error("app data directory could not be created")]
    AppDataDirectoryCreateFailed,
    #[error("diagnostics initialization failed")]
    DiagnosticsInitializationFailed,
    #[error("database initialization failed")]
    DatabaseInitializationFailed,
    #[error("bundled resource directory is unavailable")]
    ResourceDirectoryUnavailable,
    #[error("provider initialization failed")]
    ProviderInitializationFailed,
    #[error("interrupted runtime recovery failed")]
    RuntimeRecoveryFailed,
}

pub fn run() {
    let facade_builder = facade::builder();
    let app_builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(facade_builder.invoke_handler());

    app_builder
        .setup(move |app| {
            facade_builder.mount_events(app);
            bootstrap(app).map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn bootstrap(app: &mut tauri::App) -> Result<(), BootstrapError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|_| BootstrapError::AppDataDirectoryUnavailable)?;
    std::fs::create_dir_all(&app_data_dir)
        .map_err(|_| BootstrapError::AppDataDirectoryCreateFailed)?;
    diagnostics::init(&app_data_dir.join(DIAGNOSTICS_FILE_NAME))
        .map_err(|_| BootstrapError::DiagnosticsInitializationFailed)?;

    let database = tauri::async_runtime::block_on(SqliteInfraDatabase::connect(
        app_data_dir.join(DB_FILE_NAME),
    ))
    .map_err(|error| {
        log::error!("database initialization failed: {error:?}");
        BootstrapError::DatabaseInitializationFailed
    })?;
    let bundled_dir = app
        .path()
        .resource_dir()
        .map(|path| path.join(BUNDLED_DIR_NAME))
        .map_err(|error| {
            log::error!("bundled resource directory is unavailable: {error:?}");
            BootstrapError::ResourceDirectoryUnavailable
        })?;

    let bundled = Arc::new(BundledCatalogAdapter::new(bundled_dir));
    let secrets = Arc::new(KeyringSecretStore::new(app.config().identifier.clone()));
    let runpod_identity = Arc::new(RunpodIdentityAdapter::new().map_err(provider_error)?);
    let hugging_face_identity =
        Arc::new(HuggingFaceIdentityAdapter::new().map_err(provider_error)?);
    let runpod_provider = Arc::new(RunpodRuntimeProviderAdapter::new().map_err(provider_error)?);
    let connection = database.connection().clone();
    let workspaces = Arc::new(SqliteWorkspaceRepository::new(connection.clone()));
    let transitions = Arc::new(SqliteRuntimeTransitionRepository::new(connection.clone()));
    let operations = Arc::new(SqliteRuntimeOperationRepository::new(connection));
    let events = Arc::new(TauriEventSink::new(app.handle().clone()));

    let workspace_service =
        WorkspaceService::new(workspaces.clone(), bundled.clone(), events.clone());
    let secrets_service =
        SecretsService::new(secrets.clone(), runpod_identity, hugging_face_identity);
    let runpod_service = RunpodRuntimeService::new(RunpodRuntimeServiceDependencies {
        workspaces: workspaces.clone(),
        operations: operations.clone(),
        workflows: bundled.clone(),
        transitions,
        runtime_catalog: bundled,
        secrets,
        provider: runpod_provider,
        events,
    });
    let runtime_service = RuntimeService::new(workspaces, operations, runpod_service.clone());
    let facade_state = FacadeState::new(
        workspace_service,
        secrets_service,
        runtime_service,
        runpod_service,
    );

    tauri::async_runtime::block_on(facade_state.recover_interrupted()).map_err(|error| {
        log::error!("interrupted runtime recovery failed: {error:?}");
        BootstrapError::RuntimeRecoveryFailed
    })?;
    app.manage(facade_state);
    Ok(())
}

fn provider_error(error: providers::NetworkError) -> BootstrapError {
    log::error!("provider initialization failed: {error:?}");
    BootstrapError::ProviderInitializationFailed
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri_specta::Event;

    #[test]
    fn facade_event_names_are_stable() {
        assert_eq!(facade::WorkspaceChangedEvent::NAME, "workspace_changed");
        assert_eq!(facade::WorkspaceDeletedEvent::NAME, "workspace_deleted");
        assert_eq!(facade::RuntimeOperationEvent::NAME, "runtime_operation");
    }

    #[test]
    fn support_file_names_are_stable() {
        assert_eq!(DB_FILE_NAME, "db.sqlite");
        assert_eq!(DIAGNOSTICS_FILE_NAME, "diagnostics.log");
    }

    #[test]
    fn events_mount_before_interrupted_recovery() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"),
        )
        .unwrap();
        assert!(
            source.find("mount_events(app)").unwrap() < source.find("recover_interrupted").unwrap()
        );
    }

    #[test]
    fn export_bindings() {
        facade::export_typescript_bindings(&facade::builder())
            .expect("failed to export TypeScript bindings");
    }
}
