use std::sync::Arc;

use crate::{
    commands::errors::NativeCommandError,
    lifecycle_journal::sqlite::SqliteLifecycleJournalRepository,
    provider::runpod::RunpodRuntimeClient,
    runtime_catalog::BundledRuntimeCatalogRepository,
    secrets::{
        identities::{hugging_face::HuggingFaceIdentityProvider, runpod::RunpodIdentityProvider},
        stores::keyring::KeyringSecretStore,
        SecretsService,
    },
    workflow_catalog::BundledWorkflowCatalogRepository,
    workspace::WorkspaceService,
    workspace_catalog::sqlite::SqliteWorkspaceCatalogRepository,
};

pub type WorkspaceCatalogAppService = SqliteWorkspaceCatalogRepository;
pub type WorkspaceAppService = WorkspaceService<
    SqliteWorkspaceCatalogRepository,
    SqliteLifecycleJournalRepository,
    BundledWorkflowCatalogRepository,
    BundledRuntimeCatalogRepository,
>;
pub type RunpodProviderAppService = Arc<dyn RunpodRuntimeClient>;
pub type RunpodSecretsService = SecretsService<KeyringSecretStore, RunpodIdentityProvider>;
pub type HuggingFaceSecretsService =
    SecretsService<KeyringSecretStore, HuggingFaceIdentityProvider>;

pub struct AppState {
    pub workflow_catalog: BundledWorkflowCatalogRepository,
    pub runtime_catalog: BundledRuntimeCatalogRepository,
    pub workspace_catalog: WorkspaceCatalogAppService,
    pub workspace: WorkspaceAppService,
    pub runpod_provider: RunpodProviderAppService,
    pub runpod_secrets: RunpodSecretsService,
    pub hugging_face_secrets: HuggingFaceSecretsService,
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
