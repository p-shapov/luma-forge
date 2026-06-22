use serde::{Deserialize, Serialize};

use crate::{provider::errors::ProviderApiError, secrets::SecretsStorageError};

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    thiserror::Error,
    luma_diagnostic::DiagnosticCode,
)]
#[serde(rename_all = "snake_case")]
pub enum RunpodProviderError {
    #[error("provider api error")]
    ProviderApiError(#[from] ProviderApiError),
    #[error("runtime provider api key unavailable")]
    RuntimeProviderApiKeyUnavailable(#[source] SecretsStorageError),
    #[error("workflow provider api key unavailable")]
    WorkflowProviderApiKeyUnavailable(#[source] SecretsStorageError),
    #[error("provisioner worker unavailable")]
    ProvisionerWorkerUnavailable,
    #[error("provisioner worker response invalid")]
    ProvisionerWorkerResponseInvalid,
    #[error("provisioner worker failed: {message}")]
    ProvisionerWorkerFailed { message: String },
}
