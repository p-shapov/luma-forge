use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    provider_setup::ProviderSetupError, workspace_setup::workspace_setup_error::WorkspaceSetupError,
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum NativeCommandErrorCode {
    UnsupportedProvider,
    ProviderSetupIncomplete,
    ProviderSetupAlreadyExists,
    InvalidProviderApiKey,
    ProviderApiUnavailable,
    ProviderIdentityUnavailable,
    SecureKeyringUnavailable,
    ProviderSetupRecoveryRequired,
    LocalStorageUnavailable,
    WorkflowCatalogUnavailable,
    WorkspaceCatalogUnavailable,
    InvalidPlacementPlan,
    WorkspaceAlreadyExists,
    InvalidRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NativeCommandError {
    pub code: NativeCommandErrorCode,
    pub message: String,
    pub retryable: bool,
}

impl From<ProviderSetupError> for NativeCommandError {
    fn from(error: ProviderSetupError) -> Self {
        Self {
            code: provider_setup_error_code(&error),
            message: provider_setup_error_message(&error).to_string(),
            retryable: provider_setup_error_retryable(&error),
        }
    }
}

impl From<WorkspaceSetupError> for NativeCommandError {
    fn from(error: WorkspaceSetupError) -> Self {
        Self {
            code: workspace_setup_error_code(&error),
            message: workspace_setup_error_message(&error).to_string(),
            retryable: workspace_setup_error_retryable(&error),
        }
    }
}

fn provider_setup_error_code(error: &ProviderSetupError) -> NativeCommandErrorCode {
    match error {
        ProviderSetupError::ProviderSetupIncomplete => {
            NativeCommandErrorCode::ProviderSetupIncomplete
        }
        ProviderSetupError::ProviderSetupAlreadyExists => {
            NativeCommandErrorCode::ProviderSetupAlreadyExists
        }
        ProviderSetupError::InvalidProviderApiKey => NativeCommandErrorCode::InvalidProviderApiKey,
        ProviderSetupError::ProviderApiUnavailable => {
            NativeCommandErrorCode::ProviderApiUnavailable
        }
        ProviderSetupError::ProviderIdentityUnavailable => {
            NativeCommandErrorCode::ProviderIdentityUnavailable
        }
        ProviderSetupError::SecureKeyringUnavailable => {
            NativeCommandErrorCode::SecureKeyringUnavailable
        }
        ProviderSetupError::ProviderSetupRecoveryRequired => {
            NativeCommandErrorCode::ProviderSetupRecoveryRequired
        }
    }
}

fn provider_setup_error_retryable(error: &ProviderSetupError) -> bool {
    matches!(
        error,
        ProviderSetupError::ProviderApiUnavailable | ProviderSetupError::SecureKeyringUnavailable
    )
}

fn provider_setup_error_message(error: &ProviderSetupError) -> &'static str {
    match error {
        ProviderSetupError::ProviderSetupIncomplete => "GPU cloud provider setup is incomplete.",
        ProviderSetupError::ProviderSetupAlreadyExists => {
            "GPU cloud provider setup already exists."
        }
        ProviderSetupError::InvalidProviderApiKey => "Provider API key is invalid.",
        ProviderSetupError::ProviderApiUnavailable => "Provider API is unavailable.",
        ProviderSetupError::ProviderIdentityUnavailable => {
            "Provider identity could not be verified."
        }
        ProviderSetupError::SecureKeyringUnavailable => "Secure keyring is unavailable.",
        ProviderSetupError::ProviderSetupRecoveryRequired => {
            "GPU cloud provider setup requires local recovery."
        }
    }
}

fn workspace_setup_error_code(error: &WorkspaceSetupError) -> NativeCommandErrorCode {
    match error {
        WorkspaceSetupError::ProviderSetupIncomplete => {
            NativeCommandErrorCode::ProviderSetupIncomplete
        }
        WorkspaceSetupError::InvalidProviderApiKey => NativeCommandErrorCode::InvalidProviderApiKey,
        WorkspaceSetupError::ProviderApiUnavailable => {
            NativeCommandErrorCode::ProviderApiUnavailable
        }
        WorkspaceSetupError::SecureKeyringUnavailable => {
            NativeCommandErrorCode::SecureKeyringUnavailable
        }
        WorkspaceSetupError::WorkflowCatalogUnavailable => {
            NativeCommandErrorCode::WorkflowCatalogUnavailable
        }
        WorkspaceSetupError::WorkspaceCatalogUnavailable => {
            NativeCommandErrorCode::WorkspaceCatalogUnavailable
        }
        WorkspaceSetupError::LocalStorageUnavailable => {
            NativeCommandErrorCode::LocalStorageUnavailable
        }
        WorkspaceSetupError::InvalidPlacementPlan => NativeCommandErrorCode::InvalidPlacementPlan,
        WorkspaceSetupError::WorkspaceAlreadyExists => {
            NativeCommandErrorCode::WorkspaceAlreadyExists
        }
        WorkspaceSetupError::InvalidRequest => NativeCommandErrorCode::InvalidRequest,
    }
}

fn workspace_setup_error_retryable(error: &WorkspaceSetupError) -> bool {
    matches!(
        error,
        WorkspaceSetupError::ProviderApiUnavailable
            | WorkspaceSetupError::SecureKeyringUnavailable
            | WorkspaceSetupError::WorkspaceCatalogUnavailable
            | WorkspaceSetupError::LocalStorageUnavailable
    )
}

fn workspace_setup_error_message(error: &WorkspaceSetupError) -> &'static str {
    match error {
        WorkspaceSetupError::ProviderSetupIncomplete => "GPU cloud provider setup is incomplete.",
        WorkspaceSetupError::InvalidProviderApiKey => "Provider API key is invalid.",
        WorkspaceSetupError::ProviderApiUnavailable => "Provider API is unavailable.",
        WorkspaceSetupError::SecureKeyringUnavailable => "Secure keyring is unavailable.",
        WorkspaceSetupError::WorkflowCatalogUnavailable => "Workflow catalog is unavailable.",
        WorkspaceSetupError::WorkspaceCatalogUnavailable => "Workspace catalog is unavailable.",
        WorkspaceSetupError::LocalStorageUnavailable => "Local storage is unavailable.",
        WorkspaceSetupError::InvalidPlacementPlan => "Placement plan is invalid.",
        WorkspaceSetupError::WorkspaceAlreadyExists => "Workspace already exists.",
        WorkspaceSetupError::InvalidRequest => "Request is invalid.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_setup_error_mapping_is_ui_safe() {
        let error = NativeCommandError::from(ProviderSetupError::InvalidProviderApiKey);

        assert!(matches!(
            error.code,
            NativeCommandErrorCode::InvalidProviderApiKey
        ));
        assert_eq!(error.message, "Provider API key is invalid.");
        assert!(!error.message.contains("rp_"));
        assert!(!error.retryable);
    }

    #[test]
    fn provider_setup_recovery_required_mapping_is_ui_safe_and_not_retryable() {
        let error = NativeCommandError::from(ProviderSetupError::ProviderSetupRecoveryRequired);

        assert!(matches!(
            error.code,
            NativeCommandErrorCode::ProviderSetupRecoveryRequired
        ));
        assert_eq!(
            error.message,
            "GPU cloud provider setup requires local recovery."
        );
        assert!(!error.message.contains("rp_"));
        assert!(!error.retryable);
    }

    #[test]
    fn workspace_setup_error_mapping_is_ui_safe() {
        let error = NativeCommandError::from(WorkspaceSetupError::ProviderApiUnavailable);

        assert!(matches!(
            error.code,
            NativeCommandErrorCode::ProviderApiUnavailable
        ));
        assert_eq!(error.message, "Provider API is unavailable.");
        assert!(!error.message.contains("rp_"));
        assert!(error.retryable);
    }

    #[test]
    fn workspace_invalid_provider_key_mapping_is_not_retryable() {
        let error = NativeCommandError::from(WorkspaceSetupError::InvalidProviderApiKey);

        assert!(matches!(
            error.code,
            NativeCommandErrorCode::InvalidProviderApiKey
        ));
        assert_eq!(error.message, "Provider API key is invalid.");
        assert!(!error.message.contains("rp_"));
        assert!(!error.retryable);
    }
}
