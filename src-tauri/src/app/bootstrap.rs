use std::{fs, sync::Arc};

use tauri::{AppHandle, Manager};

use crate::{
    app::events::TauriProvisionedRemoteEventSink,
    commands::{NativeCommandError, NativeCommandErrorCode},
    lifecycle_journal::sqlite::SqliteLifecycleJournalRepository,
    provisioned_remote::{
        providers::runpod::RunpodProvisionedRemoteProvider,
        registry::ProvisionedRemoteProviderRegistry, service::ProvisionedRemoteService,
    },
    secrets_storage::{
        identities::{hugging_face::HuggingFaceIdentityProvider, runpod::RunpodIdentityProvider},
        stores::{keyring::KeyringSecretStore, SecretKey},
        SecretsStorageService,
    },
    sqlite::database::SqliteNativeDatabase,
    workflow_catalog::WorkflowCatalogService,
    workspace_catalog::{
        service::WorkspaceCatalogService, sqlite::SqliteWorkspaceCatalogRepository,
    },
};

use super::state::AppState;

const NATIVE_DB_FILE: &str = "native.sqlite";

pub async fn build_app_state(app_handle: &AppHandle) -> Result<AppState, NativeCommandError> {
    let app_identifier = app_handle.config().identifier.clone();
    let app_data_dir = app_handle.path().app_data_dir().map_err(|_| {
        NativeCommandError::new(
            NativeCommandErrorCode::WorkspaceStorageUnavailable,
            "app data directory is unavailable",
        )
    })?;
    fs::create_dir_all(&app_data_dir).map_err(|_| {
        NativeCommandError::new(
            NativeCommandErrorCode::WorkspaceStorageUnavailable,
            "app data directory could not be created",
        )
    })?;

    let database = SqliteNativeDatabase::connect(app_data_dir.join(NATIVE_DB_FILE))
        .await
        .map_err(|_| {
            NativeCommandError::new(
                NativeCommandErrorCode::WorkspaceStorageUnavailable,
                "workspace storage could not be initialized",
            )
        })?;
    let pool = database.pool();
    let workspace_repository = SqliteWorkspaceCatalogRepository::from_pool(pool.clone());
    let lifecycle_journal = SqliteLifecycleJournalRepository::new(pool);
    let workspace_catalog = WorkspaceCatalogService::new(workspace_repository.clone());
    let workflow_catalog = WorkflowCatalogService::new();

    let runpod_secrets = build_runpod_secrets(&app_identifier)?;
    let hugging_face_secrets = build_hugging_face_secrets(&app_identifier)?;
    let provider_runpod_secrets = build_runpod_secrets(&app_identifier)?;
    let provider_hugging_face_secrets = build_hugging_face_secrets(&app_identifier)?;

    let runpod_provider = RunpodProvisionedRemoteProvider::new(
        provider_runpod_secrets,
        provider_hugging_face_secrets,
    );
    let provider_registry = ProvisionedRemoteProviderRegistry::new(vec![Box::new(runpod_provider)]);
    let provisioned_remote =
        ProvisionedRemoteService::new(workspace_repository, lifecycle_journal, provider_registry)
            .with_event_sink(Arc::new(TauriProvisionedRemoteEventSink::new(
                app_handle.clone(),
            )));

    provisioned_remote
        .mark_running_operations_stale()
        .await
        .map_err(|_| {
            NativeCommandError::new(
                NativeCommandErrorCode::WorkspaceStorageUnavailable,
                "workspace lifecycle state could not be restored",
            )
        })?;

    Ok(AppState {
        workflow_catalog,
        workspace_catalog,
        provisioned_remote,
        runpod_secrets,
        hugging_face_secrets,
    })
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
