use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    provider::runpod::RunpodProviderError, runtime_catalog::RuntimeCatalogError,
    secrets::SecretsStorageError, shared::ApiError, workflow_catalog::WorkflowCatalogError,
    workspace_catalog::WorkspaceCatalogError,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, thiserror::Error)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::enum_variant_names)]
pub enum GetWorkflowCatalogErrorCode {
    #[error("native initialization failed")]
    NativeInitializationFailed,
    #[error("workflow catalog parse failed")]
    ParseFailed,
    #[error("workflow catalog validation failed")]
    ValidationFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, thiserror::Error)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::enum_variant_names)]
pub enum GetRuntimeContractCatalogErrorCode {
    #[error("native initialization failed")]
    NativeInitializationFailed,
    #[error("runtime catalog parse failed")]
    ParseFailed,
    #[error("runtime catalog validation failed")]
    ValidationFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum GetRunpodPlacementOptionsErrorCode {
    #[error("native initialization failed")]
    NativeInitializationFailed,
    #[error("provider request was unauthorized")]
    ProviderUnauthorized,
    #[error("provider request has insufficient permissions")]
    ProviderInsufficientPermissions,
    #[error("provider request was rate limited")]
    ProviderRateLimited,
    #[error("provider request timed out")]
    ProviderTimeout,
    #[error("provider request failed")]
    ProviderRequestFailed,
    #[error("secure storage is unavailable")]
    StoreUnavailable,
    #[error("api key is not configured")]
    KeyNotFound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum GetWorkspaceCatalogErrorCode {
    #[error("native initialization failed")]
    NativeInitializationFailed,
    #[error("workspace catalog storage unavailable")]
    StorageUnavailable,
    #[error("workspace catalog schema is invalid")]
    SchemaInvalid,
    #[error("workspace catalog data is invalid")]
    DataInvalid,
}

pub fn get_workflow_catalog_error(error: &WorkflowCatalogError) -> GetWorkflowCatalogErrorCode {
    match error {
        WorkflowCatalogError::ParseFailed { .. } => GetWorkflowCatalogErrorCode::ParseFailed,
        WorkflowCatalogError::ValidationFailed { .. } => {
            GetWorkflowCatalogErrorCode::ValidationFailed
        }
    }
}

pub fn get_runtime_contract_catalog_error(
    error: &RuntimeCatalogError,
) -> GetRuntimeContractCatalogErrorCode {
    match error {
        RuntimeCatalogError::ParseFailed { .. } => GetRuntimeContractCatalogErrorCode::ParseFailed,
        RuntimeCatalogError::ValidationFailed { .. } => {
            GetRuntimeContractCatalogErrorCode::ValidationFailed
        }
    }
}

pub fn get_runpod_placement_options_error(
    error: &RunpodProviderError,
) -> GetRunpodPlacementOptionsErrorCode {
    match error {
        RunpodProviderError::ProviderApiError(error) => provider_error(error),
        RunpodProviderError::RuntimeProviderApiKeyUnavailable(error)
        | RunpodProviderError::WorkflowProviderApiKeyUnavailable(error) => match error {
            SecretsStorageError::SecretRequired | SecretsStorageError::KeyNotFound => {
                GetRunpodPlacementOptionsErrorCode::KeyNotFound
            }
            SecretsStorageError::KeyAlreadyExists
            | SecretsStorageError::StoreUnavailable
            | SecretsStorageError::StoredSecretInvalid
            | SecretsStorageError::IdentityRequestFailed(_)
            | SecretsStorageError::IdentityResponseInvalid { .. } => {
                GetRunpodPlacementOptionsErrorCode::StoreUnavailable
            }
        },
        RunpodProviderError::ProvisionerWorkerUnavailable { .. }
        | RunpodProviderError::ProvisionerWorkerResponseInvalid { .. }
        | RunpodProviderError::ProvisionerWorkerFailed { .. } => {
            GetRunpodPlacementOptionsErrorCode::ProviderRequestFailed
        }
    }
}

pub fn get_workspace_catalog_error(error: &WorkspaceCatalogError) -> GetWorkspaceCatalogErrorCode {
    match error {
        WorkspaceCatalogError::StorageUnavailable { .. } => {
            GetWorkspaceCatalogErrorCode::StorageUnavailable
        }
        WorkspaceCatalogError::SchemaInvalid { .. } => GetWorkspaceCatalogErrorCode::SchemaInvalid,
        WorkspaceCatalogError::DataInvalid { .. } => GetWorkspaceCatalogErrorCode::DataInvalid,
        WorkspaceCatalogError::WorkspaceAlreadyExists
        | WorkspaceCatalogError::WorkspaceNotFound => GetWorkspaceCatalogErrorCode::DataInvalid,
    }
}

fn provider_error(error: &ApiError) -> GetRunpodPlacementOptionsErrorCode {
    match error {
        ApiError::Unauthorized => GetRunpodPlacementOptionsErrorCode::ProviderUnauthorized,
        ApiError::InsufficientPermissions => {
            GetRunpodPlacementOptionsErrorCode::ProviderInsufficientPermissions
        }
        ApiError::RateLimited => GetRunpodPlacementOptionsErrorCode::ProviderRateLimited,
        ApiError::Timeout => GetRunpodPlacementOptionsErrorCode::ProviderTimeout,
        ApiError::RequestFailed { .. } => GetRunpodPlacementOptionsErrorCode::ProviderRequestFailed,
    }
}
