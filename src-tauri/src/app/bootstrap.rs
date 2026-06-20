use std::sync::Arc;

use crate::{
    app::errors::AppInitializationError,
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
    shared::{BackgroundTaskSpawner, EventSink},
    sqlite::database::SqliteNativeDatabase,
    workflow_catalog::BundledWorkflowCatalogRepository,
    workspace::events::WorkspaceEvent,
    workspace::{
        WorkspaceRuntimeDispatcher, WorkspaceRuntimeImplementations, WorkspaceService,
        WorkspaceServiceDependencies,
    },
    workspace_catalog::sqlite::SqliteWorkspaceCatalogRepository,
};

use super::{state::AppState, support::SupportPaths};

pub async fn build_app_state(
    app_identifier: &str,
    support_paths: &SupportPaths,
    event_sink: Arc<dyn EventSink<WorkspaceEvent>>,
    task_spawner: Arc<dyn BackgroundTaskSpawner>,
) -> Result<AppState, AppInitializationError> {
    let native_db_path = support_paths.native_db_path();
    let database = SqliteNativeDatabase::connect(native_db_path)
        .await
        .map_err(
            |error| AppInitializationError::WorkspaceStorageInitializationFailed {
                path: native_db_path.display().to_string(),
                message: error.to_string(),
            },
        )?;
    let pool = database.pool();
    let workspace_catalog = SqliteWorkspaceCatalogRepository::new(pool.clone());
    let lifecycle_journal = SqliteLifecycleJournalRepository::new(pool);
    let workflow_catalog = BundledWorkflowCatalogRepository::new();
    let runtime_catalog = BundledRuntimeCatalogRepository::new();

    let runpod_secrets = build_runpod_secrets(app_identifier)?;
    let hugging_face_secrets = build_hugging_face_secrets(app_identifier)?;

    let runpod_provider = Arc::new(
        WorkspaceRunpodRuntimeProvider::new(runpod_secrets.clone(), hugging_face_secrets.clone())
            .map_err(native_provider_initialization_error)?,
    );
    let workspace_runtime = RunpodWorkspaceRuntime::new(
        runpod_provider.clone(),
        Arc::new(workflow_catalog.clone()),
        Arc::new(runtime_catalog.clone()),
    );
    let runtime_dispatcher = WorkspaceRuntimeDispatcher::new(WorkspaceRuntimeImplementations {
        runpod: Arc::new(workspace_runtime),
    });
    let workspace = WorkspaceService::new(WorkspaceServiceDependencies {
        workspace_catalog: Arc::new(workspace_catalog.clone()),
        lifecycle_journal: Arc::new(lifecycle_journal.clone()),
        workflow_catalog: Arc::new(workflow_catalog.clone()),
        runtime_dispatcher,
        event_sink,
        task_spawner,
    });

    workspace
        .mark_running_operations_stale()
        .await
        .map_err(
            |error| AppInitializationError::LifecycleStateRestoreFailed {
                message: error.to_string(),
            },
        )?;

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
) -> Result<SecretsService<KeyringSecretStore, RunpodIdentityProvider>, AppInitializationError> {
    Ok(SecretsService::new(
        KeyringSecretStore::new(app_identifier),
        RunpodIdentityProvider::new().map_err(native_provider_initialization_error)?,
        SecretKey::RunpodApiKey,
    ))
}

fn build_hugging_face_secrets(
    app_identifier: &str,
) -> Result<SecretsService<KeyringSecretStore, HuggingFaceIdentityProvider>, AppInitializationError>
{
    Ok(SecretsService::new(
        KeyringSecretStore::new(app_identifier),
        HuggingFaceIdentityProvider::new().map_err(native_provider_initialization_error)?,
        SecretKey::HuggingFaceApiKey,
    ))
}

fn native_provider_initialization_error(error: SecretsStorageError) -> AppInitializationError {
    AppInitializationError::ProviderServicesInitializationFailed {
        message: error.to_string(),
    }
}
