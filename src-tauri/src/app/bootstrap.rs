use std::{fs, sync::Arc};

use tauri::{AppHandle, Manager};

use crate::{
    app::{background::TauriBackgroundTaskSpawner, events::TauriWorkspaceEventSink},
    commands::errors::{NativeCommandError, NativeInitializationCommandError},
    lifecycle_journal::sqlite::SqliteLifecycleJournalRepository,
    provider::{
        hugging_face::HuggingFaceIdentityProvider,
        runpod::{
            RunpodIdentityProvider, RunpodRuntimeProvider as WorkspaceRunpodRuntimeProvider,
            RunpodWorkspaceRuntime,
        },
    },
    runtime_catalog::BundledRuntimeCatalogRepository,
    secrets::SecretsStorageError,
    secrets::{
        stores::{keyring::KeyringSecretStore, SecretKey},
        SecretsService,
    },
    sqlite::database::SqliteNativeDatabase,
    workflow_catalog::BundledWorkflowCatalogRepository,
    workspace::{WorkspaceService, WorkspaceServiceDependencies},
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
                path: app_data_dir.display().to_string(),
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
                    path: native_db_path.display().to_string(),
                    message: error.to_string(),
                },
            )
        })?;
    let pool = database.pool();
    let workspace_catalog = SqliteWorkspaceCatalogRepository::new(pool.clone());
    let lifecycle_journal = SqliteLifecycleJournalRepository::new(pool);
    let workflow_catalog = BundledWorkflowCatalogRepository::new();
    let runtime_catalog = BundledRuntimeCatalogRepository::new();

    let runpod_secrets = build_runpod_secrets(&app_identifier)?;
    let hugging_face_secrets = build_hugging_face_secrets(&app_identifier)?;

    let runpod_provider = Arc::new(
        WorkspaceRunpodRuntimeProvider::new(runpod_secrets.clone(), hugging_face_secrets.clone())
            .map_err(native_provider_initialization_error)?,
    );
    let workspace_runtime = RunpodWorkspaceRuntime::new(runpod_provider.clone());
    let workspace = WorkspaceService::new(WorkspaceServiceDependencies {
        workspace_catalog: Arc::new(workspace_catalog.clone()),
        lifecycle_journal: Arc::new(lifecycle_journal.clone()),
        workflow_catalog: workflow_catalog.clone(),
        runtime: Arc::new(workspace_runtime),
        lifecycle_operation_registry: Default::default(),
        event_sink: Arc::new(TauriWorkspaceEventSink::new(app_handle.clone())),
        task_spawner: Arc::new(TauriBackgroundTaskSpawner),
    });

    workspace
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
        lifecycle_journal,
        workspace,
        runpod_provider,
        runpod_secrets,
        hugging_face_secrets,
    })
}

fn build_runpod_secrets(
    app_identifier: &str,
) -> Result<SecretsService<KeyringSecretStore, RunpodIdentityProvider>, NativeCommandError> {
    Ok(SecretsService::new(
        KeyringSecretStore::new(app_identifier),
        RunpodIdentityProvider::new().map_err(native_provider_initialization_error)?,
        SecretKey::RunpodApiKey,
    ))
}

fn build_hugging_face_secrets(
    app_identifier: &str,
) -> Result<SecretsService<KeyringSecretStore, HuggingFaceIdentityProvider>, NativeCommandError> {
    Ok(SecretsService::new(
        KeyringSecretStore::new(app_identifier),
        HuggingFaceIdentityProvider::new().map_err(native_provider_initialization_error)?,
        SecretKey::HuggingFaceApiKey,
    ))
}

fn native_provider_initialization_error(error: SecretsStorageError) -> NativeCommandError {
    NativeCommandError::native_initialization(
        NativeInitializationCommandError::ProviderServicesInitializationFailed {
            message: error.to_string(),
        },
    )
}
