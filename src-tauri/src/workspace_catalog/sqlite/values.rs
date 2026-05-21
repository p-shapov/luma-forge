use crate::{
    domain::{
        provider_setup::GpuCloudProviderId,
        workspace::{
            ProviderResourceStatus, WorkspaceLifecycleState, WorkspaceProvisioningFailureCode,
            WorkspaceProvisioningFailureSource, WorkspaceProvisioningPhase,
            WorkspaceProvisioningRecoveryAction,
        },
    },
    workspace_setup::error::WorkspaceSetupError,
};

pub(super) fn gpu_cloud_provider_id_value(provider_id: &GpuCloudProviderId) -> &'static str {
    match provider_id {
        GpuCloudProviderId::Runpod => "runpod",
    }
}

pub(super) fn lifecycle_state_value(lifecycle_state: &WorkspaceLifecycleState) -> &'static str {
    match lifecycle_state {
        WorkspaceLifecycleState::Draft => "draft",
        WorkspaceLifecycleState::Provisioning => "provisioning",
        WorkspaceLifecycleState::Ready => "ready",
        WorkspaceLifecycleState::Failed => "failed",
    }
}

pub(super) fn provider_resource_status_value(status: &ProviderResourceStatus) -> &'static str {
    match status {
        ProviderResourceStatus::Creating => "creating",
        ProviderResourceStatus::Running => "running",
        ProviderResourceStatus::Ready => "ready",
        ProviderResourceStatus::Terminated => "terminated",
        ProviderResourceStatus::Failed => "failed",
        ProviderResourceStatus::Unknown => "unknown",
    }
}

pub(super) fn provisioning_failure_code_value(
    code: &WorkspaceProvisioningFailureCode,
) -> &'static str {
    match code {
        WorkspaceProvisioningFailureCode::ProviderResourceFailed => "provider_resource_failed",
        WorkspaceProvisioningFailureCode::ProviderResourceTerminated => {
            "provider_resource_terminated"
        }
        WorkspaceProvisioningFailureCode::ProviderResourceUnknown => "provider_resource_unknown",
        WorkspaceProvisioningFailureCode::ProviderResourceMissing => "provider_resource_missing",
        WorkspaceProvisioningFailureCode::ProviderOrphanedResources => {
            "provider_orphaned_resources"
        }
        WorkspaceProvisioningFailureCode::ProviderOperationIndeterminate => {
            "provider_operation_indeterminate"
        }
        WorkspaceProvisioningFailureCode::ProvisionerWorkerTokenMissing => {
            "provisioner_worker_token_missing"
        }
        WorkspaceProvisioningFailureCode::ProvisionerWorkerTokenInvalid => {
            "provisioner_worker_token_invalid"
        }
        WorkspaceProvisioningFailureCode::ProvisionerWorkerUnauthorized => {
            "provisioner_worker_unauthorized"
        }
        WorkspaceProvisioningFailureCode::ProvisionerWorkerResponseInvalid => {
            "provisioner_worker_response_invalid"
        }
        WorkspaceProvisioningFailureCode::ProvisionerWorkerFailed => "provisioner_worker_failed",
        WorkspaceProvisioningFailureCode::ProvisionerWorkerGitCheckoutFailed => {
            "provisioner_worker_git_checkout_failed"
        }
        WorkspaceProvisioningFailureCode::ProvisionerWorkerDependencyInstallFailed => {
            "provisioner_worker_dependency_install_failed"
        }
        WorkspaceProvisioningFailureCode::ProvisionerWorkerAssetDownloadFailed => {
            "provisioner_worker_asset_download_failed"
        }
        WorkspaceProvisioningFailureCode::ProvisionerWorkerAssetAuthRequired => {
            "provisioner_worker_asset_auth_required"
        }
        WorkspaceProvisioningFailureCode::ProvisionerWorkerPathValidationFailed => {
            "provisioner_worker_path_validation_failed"
        }
        WorkspaceProvisioningFailureCode::ProvisionerWorkerStepTimeout => {
            "provisioner_worker_step_timeout"
        }
        WorkspaceProvisioningFailureCode::ProvisionerWorkerUnexpectedError => {
            "provisioner_worker_unexpected_error"
        }
        WorkspaceProvisioningFailureCode::ReadinessValidationFailed => {
            "readiness_validation_failed"
        }
        WorkspaceProvisioningFailureCode::CancellationCleanupFailed => {
            "cancellation_cleanup_failed"
        }
        WorkspaceProvisioningFailureCode::LegacyFailure => "legacy_failure",
    }
}

pub(super) fn provisioning_failure_source_value(
    source: &WorkspaceProvisioningFailureSource,
) -> &'static str {
    match source {
        WorkspaceProvisioningFailureSource::Native => "native",
        WorkspaceProvisioningFailureSource::Provider => "provider",
        WorkspaceProvisioningFailureSource::ProviderResource => "provider_resource",
        WorkspaceProvisioningFailureSource::ProvisionerWorker => "provisioner_worker",
    }
}

pub(super) fn provisioning_phase_value(phase: &WorkspaceProvisioningPhase) -> &'static str {
    match phase {
        WorkspaceProvisioningPhase::NotStarted => "not_started",
        WorkspaceProvisioningPhase::CreatingVolume => "creating_volume",
        WorkspaceProvisioningPhase::StartingProvisioningPod => "starting_provisioning_pod",
        WorkspaceProvisioningPhase::PreparingEnvironment => "preparing_environment",
        WorkspaceProvisioningPhase::CreatingEndpointTemplate => "creating_endpoint_template",
        WorkspaceProvisioningPhase::CreatingEndpoint => "creating_endpoint",
        WorkspaceProvisioningPhase::ValidatingReadiness => "validating_readiness",
        WorkspaceProvisioningPhase::CleaningUp => "cleaning_up",
        WorkspaceProvisioningPhase::Completed => "completed",
        WorkspaceProvisioningPhase::Failed => "failed",
    }
}

pub(super) fn provisioning_recovery_action_value(
    action: &WorkspaceProvisioningRecoveryAction,
) -> &'static str {
    match action {
        WorkspaceProvisioningRecoveryAction::Retry => "retry",
        WorkspaceProvisioningRecoveryAction::RecoverProviderSetup => "recover_provider_setup",
        WorkspaceProvisioningRecoveryAction::ReselectPlacement => "reselect_placement",
        WorkspaceProvisioningRecoveryAction::InspectWorkspaceProvisioning => {
            "inspect_workspace_provisioning"
        }
        WorkspaceProvisioningRecoveryAction::CleanupWorkspaceResources => {
            "cleanup_workspace_resources"
        }
    }
}

pub(super) fn parse_gpu_cloud_provider_id(
    value: &str,
) -> Result<GpuCloudProviderId, WorkspaceSetupError> {
    match value {
        "runpod" => Ok(GpuCloudProviderId::Runpod),
        _ => Err(WorkspaceSetupError::WorkspaceCatalogSchemaMismatch),
    }
}

pub(super) fn parse_lifecycle_state(
    value: &str,
) -> Result<WorkspaceLifecycleState, WorkspaceSetupError> {
    match value {
        "draft" => Ok(WorkspaceLifecycleState::Draft),
        "provisioning" => Ok(WorkspaceLifecycleState::Provisioning),
        "ready" => Ok(WorkspaceLifecycleState::Ready),
        "failed" => Ok(WorkspaceLifecycleState::Failed),
        _ => Err(WorkspaceSetupError::WorkspaceCatalogSchemaMismatch),
    }
}

pub(super) fn parse_provider_resource_status(
    value: &str,
) -> Result<ProviderResourceStatus, WorkspaceSetupError> {
    match value {
        "creating" => Ok(ProviderResourceStatus::Creating),
        "running" => Ok(ProviderResourceStatus::Running),
        "ready" => Ok(ProviderResourceStatus::Ready),
        "terminated" => Ok(ProviderResourceStatus::Terminated),
        "failed" => Ok(ProviderResourceStatus::Failed),
        "unknown" => Ok(ProviderResourceStatus::Unknown),
        _ => Err(WorkspaceSetupError::WorkspaceCatalogSchemaMismatch),
    }
}

pub(super) fn parse_provisioning_failure_code(
    value: &str,
) -> Result<WorkspaceProvisioningFailureCode, WorkspaceSetupError> {
    match value {
        "provider_resource_failed" => Ok(WorkspaceProvisioningFailureCode::ProviderResourceFailed),
        "provider_resource_terminated" => {
            Ok(WorkspaceProvisioningFailureCode::ProviderResourceTerminated)
        }
        "provider_resource_unknown" => {
            Ok(WorkspaceProvisioningFailureCode::ProviderResourceUnknown)
        }
        "provider_resource_missing" => {
            Ok(WorkspaceProvisioningFailureCode::ProviderResourceMissing)
        }
        "provider_orphaned_resources" => {
            Ok(WorkspaceProvisioningFailureCode::ProviderOrphanedResources)
        }
        "provider_operation_indeterminate" => {
            Ok(WorkspaceProvisioningFailureCode::ProviderOperationIndeterminate)
        }
        "provisioner_worker_token_missing" => {
            Ok(WorkspaceProvisioningFailureCode::ProvisionerWorkerTokenMissing)
        }
        "provisioner_worker_token_invalid" => {
            Ok(WorkspaceProvisioningFailureCode::ProvisionerWorkerTokenInvalid)
        }
        "provisioner_worker_unauthorized" => {
            Ok(WorkspaceProvisioningFailureCode::ProvisionerWorkerUnauthorized)
        }
        "provisioner_worker_response_invalid" => {
            Ok(WorkspaceProvisioningFailureCode::ProvisionerWorkerResponseInvalid)
        }
        "provisioner_worker_failed" => {
            Ok(WorkspaceProvisioningFailureCode::ProvisionerWorkerFailed)
        }
        "provisioner_worker_git_checkout_failed" => {
            Ok(WorkspaceProvisioningFailureCode::ProvisionerWorkerGitCheckoutFailed)
        }
        "provisioner_worker_dependency_install_failed" => {
            Ok(WorkspaceProvisioningFailureCode::ProvisionerWorkerDependencyInstallFailed)
        }
        "provisioner_worker_asset_download_failed" => {
            Ok(WorkspaceProvisioningFailureCode::ProvisionerWorkerAssetDownloadFailed)
        }
        "provisioner_worker_asset_auth_required" => {
            Ok(WorkspaceProvisioningFailureCode::ProvisionerWorkerAssetAuthRequired)
        }
        "provisioner_worker_path_validation_failed" => {
            Ok(WorkspaceProvisioningFailureCode::ProvisionerWorkerPathValidationFailed)
        }
        "provisioner_worker_step_timeout" => {
            Ok(WorkspaceProvisioningFailureCode::ProvisionerWorkerStepTimeout)
        }
        "provisioner_worker_unexpected_error" => {
            Ok(WorkspaceProvisioningFailureCode::ProvisionerWorkerUnexpectedError)
        }
        "readiness_validation_failed" => {
            Ok(WorkspaceProvisioningFailureCode::ReadinessValidationFailed)
        }
        "cancellation_cleanup_failed" => {
            Ok(WorkspaceProvisioningFailureCode::CancellationCleanupFailed)
        }
        "legacy_failure" => Ok(WorkspaceProvisioningFailureCode::LegacyFailure),
        _ => Err(WorkspaceSetupError::WorkspaceCatalogSchemaMismatch),
    }
}

pub(super) fn parse_provisioning_failure_source(
    value: &str,
) -> Result<WorkspaceProvisioningFailureSource, WorkspaceSetupError> {
    match value {
        "native" => Ok(WorkspaceProvisioningFailureSource::Native),
        "provider" => Ok(WorkspaceProvisioningFailureSource::Provider),
        "provider_resource" => Ok(WorkspaceProvisioningFailureSource::ProviderResource),
        "provisioner_worker" => Ok(WorkspaceProvisioningFailureSource::ProvisionerWorker),
        _ => Err(WorkspaceSetupError::WorkspaceCatalogSchemaMismatch),
    }
}

pub(super) fn parse_provisioning_phase(
    value: &str,
) -> Result<WorkspaceProvisioningPhase, WorkspaceSetupError> {
    match value {
        "not_started" => Ok(WorkspaceProvisioningPhase::NotStarted),
        "creating_volume" => Ok(WorkspaceProvisioningPhase::CreatingVolume),
        "starting_provisioning_pod" => Ok(WorkspaceProvisioningPhase::StartingProvisioningPod),
        "preparing_environment" => Ok(WorkspaceProvisioningPhase::PreparingEnvironment),
        "creating_endpoint_template" => Ok(WorkspaceProvisioningPhase::CreatingEndpointTemplate),
        "creating_endpoint" => Ok(WorkspaceProvisioningPhase::CreatingEndpoint),
        "validating_readiness" => Ok(WorkspaceProvisioningPhase::ValidatingReadiness),
        "cleaning_up" => Ok(WorkspaceProvisioningPhase::CleaningUp),
        "completed" => Ok(WorkspaceProvisioningPhase::Completed),
        "failed" => Ok(WorkspaceProvisioningPhase::Failed),
        _ => Err(WorkspaceSetupError::WorkspaceCatalogSchemaMismatch),
    }
}

pub(super) fn parse_provisioning_recovery_action(
    value: &str,
) -> Result<WorkspaceProvisioningRecoveryAction, WorkspaceSetupError> {
    match value {
        "retry" => Ok(WorkspaceProvisioningRecoveryAction::Retry),
        "recover_provider_setup" => Ok(WorkspaceProvisioningRecoveryAction::RecoverProviderSetup),
        "reselect_placement" => Ok(WorkspaceProvisioningRecoveryAction::ReselectPlacement),
        "inspect_workspace_provisioning" => {
            Ok(WorkspaceProvisioningRecoveryAction::InspectWorkspaceProvisioning)
        }
        "cleanup_workspace_resources" => {
            Ok(WorkspaceProvisioningRecoveryAction::CleanupWorkspaceResources)
        }
        _ => Err(WorkspaceSetupError::WorkspaceCatalogSchemaMismatch),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_and_lifecycle_values_round_trip() {
        assert_eq!(
            gpu_cloud_provider_id_value(&GpuCloudProviderId::Runpod),
            "runpod"
        );
        assert_eq!(
            parse_gpu_cloud_provider_id("runpod"),
            Ok(GpuCloudProviderId::Runpod)
        );
        assert_eq!(
            parse_gpu_cloud_provider_id("other"),
            Err(WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)
        );

        for (state, value) in [
            (WorkspaceLifecycleState::Draft, "draft"),
            (WorkspaceLifecycleState::Provisioning, "provisioning"),
            (WorkspaceLifecycleState::Ready, "ready"),
            (WorkspaceLifecycleState::Failed, "failed"),
        ] {
            assert_eq!(lifecycle_state_value(&state), value);
            assert_eq!(parse_lifecycle_state(value), Ok(state));
        }
        assert_eq!(
            parse_lifecycle_state("unknown"),
            Err(WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)
        );
    }

    #[test]
    fn provider_resource_status_values_round_trip() {
        for (status, value) in [
            (ProviderResourceStatus::Creating, "creating"),
            (ProviderResourceStatus::Running, "running"),
            (ProviderResourceStatus::Ready, "ready"),
            (ProviderResourceStatus::Terminated, "terminated"),
            (ProviderResourceStatus::Failed, "failed"),
            (ProviderResourceStatus::Unknown, "unknown"),
        ] {
            assert_eq!(provider_resource_status_value(&status), value);
            assert_eq!(parse_provider_resource_status(value), Ok(status));
        }
        assert_eq!(
            parse_provider_resource_status("other"),
            Err(WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)
        );
    }

    #[test]
    fn provisioning_failure_code_values_round_trip() {
        for (code, value) in [
            (
                WorkspaceProvisioningFailureCode::ProviderResourceFailed,
                "provider_resource_failed",
            ),
            (
                WorkspaceProvisioningFailureCode::ProviderResourceTerminated,
                "provider_resource_terminated",
            ),
            (
                WorkspaceProvisioningFailureCode::ProviderResourceUnknown,
                "provider_resource_unknown",
            ),
            (
                WorkspaceProvisioningFailureCode::ProviderResourceMissing,
                "provider_resource_missing",
            ),
            (
                WorkspaceProvisioningFailureCode::ProviderOrphanedResources,
                "provider_orphaned_resources",
            ),
            (
                WorkspaceProvisioningFailureCode::ProviderOperationIndeterminate,
                "provider_operation_indeterminate",
            ),
            (
                WorkspaceProvisioningFailureCode::ProvisionerWorkerTokenMissing,
                "provisioner_worker_token_missing",
            ),
            (
                WorkspaceProvisioningFailureCode::ProvisionerWorkerTokenInvalid,
                "provisioner_worker_token_invalid",
            ),
            (
                WorkspaceProvisioningFailureCode::ProvisionerWorkerUnauthorized,
                "provisioner_worker_unauthorized",
            ),
            (
                WorkspaceProvisioningFailureCode::ProvisionerWorkerResponseInvalid,
                "provisioner_worker_response_invalid",
            ),
            (
                WorkspaceProvisioningFailureCode::ProvisionerWorkerFailed,
                "provisioner_worker_failed",
            ),
            (
                WorkspaceProvisioningFailureCode::ProvisionerWorkerGitCheckoutFailed,
                "provisioner_worker_git_checkout_failed",
            ),
            (
                WorkspaceProvisioningFailureCode::ProvisionerWorkerDependencyInstallFailed,
                "provisioner_worker_dependency_install_failed",
            ),
            (
                WorkspaceProvisioningFailureCode::ProvisionerWorkerAssetDownloadFailed,
                "provisioner_worker_asset_download_failed",
            ),
            (
                WorkspaceProvisioningFailureCode::ProvisionerWorkerAssetAuthRequired,
                "provisioner_worker_asset_auth_required",
            ),
            (
                WorkspaceProvisioningFailureCode::ProvisionerWorkerPathValidationFailed,
                "provisioner_worker_path_validation_failed",
            ),
            (
                WorkspaceProvisioningFailureCode::ProvisionerWorkerStepTimeout,
                "provisioner_worker_step_timeout",
            ),
            (
                WorkspaceProvisioningFailureCode::ProvisionerWorkerUnexpectedError,
                "provisioner_worker_unexpected_error",
            ),
            (
                WorkspaceProvisioningFailureCode::ReadinessValidationFailed,
                "readiness_validation_failed",
            ),
            (
                WorkspaceProvisioningFailureCode::CancellationCleanupFailed,
                "cancellation_cleanup_failed",
            ),
            (
                WorkspaceProvisioningFailureCode::LegacyFailure,
                "legacy_failure",
            ),
        ] {
            assert_eq!(provisioning_failure_code_value(&code), value);
            assert_eq!(parse_provisioning_failure_code(value), Ok(code));
        }
        assert_eq!(
            parse_provisioning_failure_code("other"),
            Err(WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)
        );
    }

    #[test]
    fn provisioning_failure_source_values_round_trip() {
        for (source, value) in [
            (WorkspaceProvisioningFailureSource::Native, "native"),
            (WorkspaceProvisioningFailureSource::Provider, "provider"),
            (
                WorkspaceProvisioningFailureSource::ProviderResource,
                "provider_resource",
            ),
            (
                WorkspaceProvisioningFailureSource::ProvisionerWorker,
                "provisioner_worker",
            ),
        ] {
            assert_eq!(provisioning_failure_source_value(&source), value);
            assert_eq!(parse_provisioning_failure_source(value), Ok(source));
        }
        assert_eq!(
            parse_provisioning_failure_source("other"),
            Err(WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)
        );
    }

    #[test]
    fn provisioning_phase_values_round_trip() {
        for (phase, value) in [
            (WorkspaceProvisioningPhase::NotStarted, "not_started"),
            (
                WorkspaceProvisioningPhase::CreatingVolume,
                "creating_volume",
            ),
            (
                WorkspaceProvisioningPhase::StartingProvisioningPod,
                "starting_provisioning_pod",
            ),
            (
                WorkspaceProvisioningPhase::PreparingEnvironment,
                "preparing_environment",
            ),
            (
                WorkspaceProvisioningPhase::CreatingEndpointTemplate,
                "creating_endpoint_template",
            ),
            (
                WorkspaceProvisioningPhase::CreatingEndpoint,
                "creating_endpoint",
            ),
            (
                WorkspaceProvisioningPhase::ValidatingReadiness,
                "validating_readiness",
            ),
            (WorkspaceProvisioningPhase::CleaningUp, "cleaning_up"),
            (WorkspaceProvisioningPhase::Completed, "completed"),
            (WorkspaceProvisioningPhase::Failed, "failed"),
        ] {
            assert_eq!(provisioning_phase_value(&phase), value);
            assert_eq!(parse_provisioning_phase(value), Ok(phase));
        }
        assert_eq!(
            parse_provisioning_phase("other"),
            Err(WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)
        );
    }

    #[test]
    fn provisioning_recovery_action_values_round_trip() {
        for (action, value) in [
            (WorkspaceProvisioningRecoveryAction::Retry, "retry"),
            (
                WorkspaceProvisioningRecoveryAction::RecoverProviderSetup,
                "recover_provider_setup",
            ),
            (
                WorkspaceProvisioningRecoveryAction::ReselectPlacement,
                "reselect_placement",
            ),
            (
                WorkspaceProvisioningRecoveryAction::InspectWorkspaceProvisioning,
                "inspect_workspace_provisioning",
            ),
            (
                WorkspaceProvisioningRecoveryAction::CleanupWorkspaceResources,
                "cleanup_workspace_resources",
            ),
        ] {
            assert_eq!(provisioning_recovery_action_value(&action), value);
            assert_eq!(parse_provisioning_recovery_action(value), Ok(action));
        }
        assert_eq!(
            parse_provisioning_recovery_action("other"),
            Err(WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)
        );
    }
}
