use serde::{Deserialize, Serialize};

use crate::{secrets::SecretsStorageError, shared::ApiError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum RunpodProviderError {
    #[error("provider api error")]
    ProviderApiError(#[from] ApiError),
    #[error("runtime provider api key unavailable: {0}")]
    RuntimeProviderApiKeyUnavailable(#[source] SecretsStorageError),
    #[error("workflow provider api key unavailable: {0}")]
    WorkflowProviderApiKeyUnavailable(#[source] SecretsStorageError),
    #[error("provisioner worker unavailable: {message}")]
    ProvisionerWorkerUnavailable { message: String },
    #[error("provisioner worker response invalid: {message}")]
    ProvisionerWorkerResponseInvalid { message: String },
    #[error("provisioner worker failed: {message}")]
    ProvisionerWorkerFailed { message: String },
}
