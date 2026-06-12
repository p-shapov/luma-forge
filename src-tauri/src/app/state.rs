use crate::{
    lifecycle_journal::sqlite::SqliteLifecycleJournalRepository,
    provisioned_remote::service::RunpodRuntimeService,
    secrets_storage::{
        identities::{hugging_face::HuggingFaceIdentityProvider, runpod::RunpodIdentityProvider},
        stores::keyring::KeyringSecretStore,
        SecretsStorageService,
    },
    workflow_catalog::WorkflowCatalogService,
    workspace_catalog::{
        service::WorkspaceCatalogService, sqlite::SqliteWorkspaceCatalogRepository,
    },
};

pub type WorkspaceCatalogAppService = WorkspaceCatalogService<SqliteWorkspaceCatalogRepository>;
pub type RunpodRuntimeAppService =
    RunpodRuntimeService<SqliteWorkspaceCatalogRepository, SqliteLifecycleJournalRepository>;
pub type RunpodSecretsService = SecretsStorageService<KeyringSecretStore, RunpodIdentityProvider>;
pub type HuggingFaceSecretsService =
    SecretsStorageService<KeyringSecretStore, HuggingFaceIdentityProvider>;

pub struct AppState {
    pub workflow_catalog: WorkflowCatalogService,
    pub workspace_catalog: WorkspaceCatalogAppService,
    pub runpod_runtime: RunpodRuntimeAppService,
    pub runpod_secrets: RunpodSecretsService,
    pub hugging_face_secrets: HuggingFaceSecretsService,
}
