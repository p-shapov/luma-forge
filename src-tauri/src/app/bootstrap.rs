use std::{fs, path::Path, sync::Arc};

use tauri::{AppHandle, Manager};

use crate::{
    app::{background::TauriBackgroundTaskSpawner, events::TauriRunpodRuntimeEventSink},
    commands::errors::{NativeCommandError, NativeInitializationCommandError},
    lifecycle_journal::sqlite::SqliteLifecycleJournalRepository,
    runpod_runtime::{
        lifecycle::runner::BackgroundRunpodRuntimeLifecycleRunner,
        provider::RunpodRuntimeProvider,
        service::{RunpodRuntimeService, RunpodRuntimeServiceDependencies},
    },
    runtime_catalog::RuntimeCatalogService,
    secrets_storage::{
        identities::{hugging_face::HuggingFaceIdentityProvider, runpod::RunpodIdentityProvider},
        stores::{keyring::KeyringSecretStore, SecretKey},
        SecretsStorageService,
    },
    sqlite::database::SqliteNativeDatabase,
    workflow_catalog::WorkflowCatalogService,
    workspace_catalog::sqlite::SqliteWorkspaceCatalogRepository,
};

use super::state::AppState;

const NATIVE_DB_FILE: &str = "native.sqlite";

pub async fn build_app_state(app_handle: &AppHandle) -> Result<AppState, NativeCommandError> {
    let app_identifier = app_handle.config().identifier.clone();
    let app_data_dir = app_handle.path().app_data_dir().map_err(|error| {
        NativeCommandError::native_initialization(
            NativeInitializationCommandError::AppDataDirectoryUnavailable {
                message: error.to_string(),
            },
        )
    })?;
    fs::create_dir_all(&app_data_dir).map_err(|error| {
        NativeCommandError::native_initialization(
            NativeInitializationCommandError::AppDataDirectoryCreateFailed {
                path: display_path(&app_data_dir),
                message: error.to_string(),
            },
        )
    })?;

    let native_db_path = app_data_dir.join(NATIVE_DB_FILE);
    let database = SqliteNativeDatabase::connect(&native_db_path)
        .await
        .map_err(|error| {
            NativeCommandError::native_initialization(
                NativeInitializationCommandError::WorkspaceStorageInitializationFailed {
                    path: display_path(&native_db_path),
                    message: error.to_string(),
                },
            )
        })?;
    let pool = database.pool();
    let workspace_catalog = SqliteWorkspaceCatalogRepository::new(pool.clone());
    let lifecycle_journal = SqliteLifecycleJournalRepository::new(pool);
    let workflow_catalog = WorkflowCatalogService::new();
    let runtime_catalog = RuntimeCatalogService::new();

    let runpod_secrets = build_runpod_secrets(&app_identifier)?;
    let hugging_face_secrets = build_hugging_face_secrets(&app_identifier)?;
    let runtime_runpod_secrets = build_runpod_secrets(&app_identifier)?;
    let runtime_hugging_face_secrets = build_hugging_face_secrets(&app_identifier)?;

    let runpod_provider =
        RunpodRuntimeProvider::new(runtime_runpod_secrets, runtime_hugging_face_secrets);
    let runpod_runtime = RunpodRuntimeService::new(RunpodRuntimeServiceDependencies {
        workspace_catalog: workspace_catalog.clone(),
        lifecycle_journal,
        workflow_catalog: workflow_catalog.clone(),
        runtime_catalog: runtime_catalog.clone(),
        runpod_client: Arc::new(runpod_provider),
        event_sink: Arc::new(TauriRunpodRuntimeEventSink::new(app_handle.clone())),
        task_spawner: Arc::new(TauriBackgroundTaskSpawner),
        lifecycle_runner: Arc::new(BackgroundRunpodRuntimeLifecycleRunner),
    });

    runpod_runtime
        .mark_running_operations_stale()
        .await
        .map_err(|error| {
            NativeCommandError::native_initialization(
                NativeInitializationCommandError::LifecycleStateRestoreFailed {
                    message: error.to_string(),
                },
            )
        })?;

    Ok(AppState {
        workflow_catalog,
        runtime_catalog,
        workspace_catalog,
        runpod_runtime,
        runpod_secrets,
        hugging_face_secrets,
    })
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

fn build_runpod_secrets(
    app_identifier: &str,
) -> Result<SecretsStorageService<KeyringSecretStore, RunpodIdentityProvider>, NativeCommandError> {
    Ok(SecretsStorageService::new(
        KeyringSecretStore::new(app_identifier),
        RunpodIdentityProvider::try_new_default()?,
        SecretKey::RunpodApiKey,
    ))
}

fn build_hugging_face_secrets(
    app_identifier: &str,
) -> Result<
    SecretsStorageService<KeyringSecretStore, HuggingFaceIdentityProvider>,
    NativeCommandError,
> {
    Ok(SecretsStorageService::new(
        KeyringSecretStore::new(app_identifier),
        HuggingFaceIdentityProvider::try_new_default()?,
        SecretKey::HuggingFaceApiKey,
    ))
}
