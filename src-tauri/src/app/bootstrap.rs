use std::fs;

use tauri::{AppHandle, Manager};

use crate::{
    commands::{NativeCommandError, NativeCommandErrorCode},
    provisioned_remote_compute::{
        providers::runpod::RunpodProvisionedRemoteComputeProvider,
        registry::ProvisionedRemoteComputeProviderRegistry,
        service::ProvisionedRemoteComputeService,
    },
    secrets_storage::{
        identities::{hugging_face::HuggingFaceIdentityProvider, runpod::RunpodIdentityProvider},
        stores::{keyring::KeyringSecretStore, SecretKey},
        SecretsStorageService,
    },
    workflow_catalog::WorkflowCatalogService,
    workspace_catalog::{
        service::WorkspaceCatalogService, sqlite::SqliteWorkspaceCatalogRepository,
    },
};

use super::state::AppState;

const WORKSPACE_CATALOG_DB_FILE: &str = "workspace-catalog.sqlite";

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

    let workspace_repository =
        SqliteWorkspaceCatalogRepository::connect(app_data_dir.join(WORKSPACE_CATALOG_DB_FILE))
            .await?;
    let workspace_catalog = WorkspaceCatalogService::new(workspace_repository);
    let workflow_catalog = WorkflowCatalogService::new();

    let runpod_secrets = build_runpod_secrets(&app_identifier)?;
    let hugging_face_secrets = build_hugging_face_secrets(&app_identifier)?;
    let provider_runpod_secrets = build_runpod_secrets(&app_identifier)?;
    let provider_hugging_face_secrets = build_hugging_face_secrets(&app_identifier)?;

    let runpod_provider = RunpodProvisionedRemoteComputeProvider::new(
        provider_runpod_secrets,
        provider_hugging_face_secrets,
    );
    let provider_registry =
        ProvisionedRemoteComputeProviderRegistry::new(vec![Box::new(runpod_provider)]);
    let provisioned_remote_compute =
        ProvisionedRemoteComputeService::new(provider_registry, workflow_catalog.clone());

    Ok(AppState {
        workflow_catalog,
        workspace_catalog,
        provisioned_remote_compute,
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
