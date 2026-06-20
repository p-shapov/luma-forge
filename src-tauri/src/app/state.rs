use std::sync::Arc;

use crate::{
    commands::errors::NativeCommandError,
    lifecycle_journal::sqlite::SqliteLifecycleJournalRepository,
    provider::{
        hugging_face::HuggingFaceIdentityProvider,
        runpod::{RunpodIdentityProvider, RunpodRuntimeClient},
    },
    runtime_catalog::BundledRuntimeCatalogRepository,
    secrets::{stores::keyring::KeyringSecretStore, SecretsService},
    workflow_catalog::BundledWorkflowCatalogRepository,
    workspace::WorkspaceService,
    workspace_catalog::sqlite::SqliteWorkspaceCatalogRepository,
};

pub struct AppState {
    pub workflow_catalog: BundledWorkflowCatalogRepository,
    pub runtime_catalog: BundledRuntimeCatalogRepository,
    pub workspace_catalog: SqliteWorkspaceCatalogRepository,
    pub lifecycle_journal: SqliteLifecycleJournalRepository,
    pub workspace: WorkspaceService,
    pub runpod_provider: Arc<dyn RunpodRuntimeClient>,
    pub runpod_secrets: SecretsService<KeyringSecretStore, RunpodIdentityProvider>,
    pub hugging_face_secrets: SecretsService<KeyringSecretStore, HuggingFaceIdentityProvider>,
}

pub enum NativeAppState {
    Ready(Box<AppState>),
    Failed(NativeCommandError),
}

impl NativeAppState {
    pub fn ready(&self) -> Result<&AppState, NativeCommandError> {
        match self {
            Self::Ready(state) => Ok(state),
            Self::Failed(error) => Err(error.clone()),
        }
    }

    pub fn startup_error(&self) -> Option<&NativeCommandError> {
        match self {
            Self::Ready(_) => None,
            Self::Failed(error) => Some(error),
        }
    }
}
