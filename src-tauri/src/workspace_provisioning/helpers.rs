use thiserror::Error;

use crate::{
    domain::workspace::{
        PersistentStorageVolumeSnapshot, ProviderResourceStatus, ProvisioningPodSnapshot,
        ServerlessEndpointSnapshot, Workspace, WorkspaceLifecycleState, WorkspaceProvisioningPhase,
        WorkspaceProvisioningProgress, WorkspaceProvisioningStatus,
    },
    secrets::SecretStoreError,
    workspace_resources::{
        NetworkVolumeObservation, ProvisioningPodObservation, ServerlessEndpointObservation,
        WorkspaceResourceError,
    },
    workspace_setup::error::WorkspaceSetupError,
};

use super::{
    failure::legacy_failure,
    gateway::{
        ProvisionerWorkerError, ProvisionerWorkerHttpGatewayInitError, ProvisionerWorkerJobStatus,
        ProvisionerWorkerPhase, ProvisionerWorkerStatus,
    },
    readiness::has_ready_matching_endpoint_template,
};

const PROGRESS_NOT_STARTED: u8 = 0;
const PROGRESS_CREATING_VOLUME: u8 = 0;
const PROGRESS_STARTING_PROVISIONING_POD: u8 = 10;
const PROGRESS_PREPARING_ENVIRONMENT_START: u8 = 40;
const PROGRESS_PREPARING_ENVIRONMENT_END: u8 = 90;
const PROGRESS_CREATING_ENDPOINT: u8 = 90;
const PROGRESS_VALIDATING_READINESS: u8 = 98;
const PROGRESS_COMPLETED: u8 = 100;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum WorkspaceProvisioningError {
    #[error("workspace not found")]
    WorkspaceNotFound,
    #[error("invalid workspace lifecycle")]
    InvalidWorkspaceLifecycle,
    #[error("workspace catalog unavailable")]
    WorkspaceCatalogUnavailable,
    #[error("workspace catalog storage unavailable")]
    WorkspaceCatalogStorageUnavailable,
    #[error("workspace catalog migration failed")]
    WorkspaceCatalogMigrationFailed,
    #[error("workspace catalog query failed")]
    WorkspaceCatalogQueryFailed,
    #[error("workspace catalog corrupt")]
    WorkspaceCatalogCorrupt,
    #[error("workspace catalog schema mismatch")]
    WorkspaceCatalogSchemaMismatch,
    #[error("provider setup is incomplete")]
    ProviderSetupIncomplete,
    #[error("provider api key unauthorized")]
    ProviderApiKeyUnauthorized,
    #[error("provider api unavailable")]
    ProviderApiUnavailable,
    #[error("provider rate limited")]
    ProviderRateLimited,
    #[error("provider request rejected")]
    ProviderRequestRejected,
    #[error("provider response invalid")]
    ProviderResponseInvalid,
    #[error("provider resource not found")]
    ProviderResourceNotFound,
    #[error("provider operation conflict")]
    ProviderOperationConflict,
    #[error("provider operation indeterminate")]
    ProviderOperationIndeterminate,
    #[error("secure keyring unavailable")]
    SecureKeyringUnavailable,
    #[error("provisioner worker token invalid")]
    ProvisionerWorkerTokenInvalid,
    #[error("provisioner worker unauthorized")]
    ProvisionerWorkerUnauthorized,
    #[error("provisioner worker unavailable")]
    ProvisionerWorkerUnavailable,
    #[error("provisioner worker conflict")]
    ProvisionerWorkerConflict,
    #[error("provisioner worker response invalid")]
    ProvisionerWorkerResponseInvalid,
    #[error("provisioner worker failed")]
    ProvisionerWorkerFailed,
}

impl From<SecretStoreError> for WorkspaceProvisioningError {
    fn from(error: SecretStoreError) -> Self {
        match error {
            SecretStoreError::SecureKeyringUnavailable => Self::SecureKeyringUnavailable,
            SecretStoreError::InvalidStoredProviderApiKey => Self::ProviderSetupIncomplete,
            SecretStoreError::InvalidStoredProvisionerWorkerToken => {
                Self::ProvisionerWorkerTokenInvalid
            }
        }
    }
}

impl From<WorkspaceResourceError> for WorkspaceProvisioningError {
    fn from(error: WorkspaceResourceError) -> Self {
        match error {
            WorkspaceResourceError::WorkspaceCatalogUnavailable => {
                Self::WorkspaceCatalogUnavailable
            }
            WorkspaceResourceError::WorkspaceCatalogStorageUnavailable => {
                Self::WorkspaceCatalogStorageUnavailable
            }
            WorkspaceResourceError::WorkspaceCatalogMigrationFailed => {
                Self::WorkspaceCatalogMigrationFailed
            }
            WorkspaceResourceError::WorkspaceCatalogQueryFailed => {
                Self::WorkspaceCatalogQueryFailed
            }
            WorkspaceResourceError::WorkspaceCatalogCorrupt => Self::WorkspaceCatalogCorrupt,
            WorkspaceResourceError::WorkspaceCatalogSchemaMismatch => {
                Self::WorkspaceCatalogSchemaMismatch
            }
            WorkspaceResourceError::ProviderSetupIncomplete => Self::ProviderSetupIncomplete,
            WorkspaceResourceError::ProviderApiKeyUnauthorized => Self::ProviderApiKeyUnauthorized,
            WorkspaceResourceError::ProviderApiUnavailable => Self::ProviderApiUnavailable,
            WorkspaceResourceError::ProviderRateLimited => Self::ProviderRateLimited,
            WorkspaceResourceError::ProviderRequestRejected => Self::ProviderRequestRejected,
            WorkspaceResourceError::ProviderResponseInvalid => Self::ProviderResponseInvalid,
            WorkspaceResourceError::ProviderResourceNotFound => Self::ProviderResourceNotFound,
            WorkspaceResourceError::ProviderOperationConflict => Self::ProviderOperationConflict,
            WorkspaceResourceError::ProviderOperationIndeterminate => {
                Self::ProviderOperationIndeterminate
            }
            WorkspaceResourceError::SecureKeyringUnavailable => Self::SecureKeyringUnavailable,
            WorkspaceResourceError::ProvisionerWorkerTokenInvalid => {
                Self::ProvisionerWorkerTokenInvalid
            }
        }
    }
}

pub(crate) fn catalog_error(error: WorkspaceSetupError) -> WorkspaceProvisioningError {
    match error {
        WorkspaceSetupError::WorkspaceCatalogUnavailable => {
            WorkspaceProvisioningError::WorkspaceCatalogUnavailable
        }
        WorkspaceSetupError::WorkspaceCatalogStorageUnavailable => {
            WorkspaceProvisioningError::WorkspaceCatalogStorageUnavailable
        }
        WorkspaceSetupError::WorkspaceCatalogMigrationFailed => {
            WorkspaceProvisioningError::WorkspaceCatalogMigrationFailed
        }
        WorkspaceSetupError::WorkspaceCatalogQueryFailed => {
            WorkspaceProvisioningError::WorkspaceCatalogQueryFailed
        }
        WorkspaceSetupError::WorkspaceCatalogCorrupt => {
            WorkspaceProvisioningError::WorkspaceCatalogCorrupt
        }
        WorkspaceSetupError::WorkspaceCatalogSchemaMismatch => {
            WorkspaceProvisioningError::WorkspaceCatalogSchemaMismatch
        }
        _ => WorkspaceProvisioningError::WorkspaceCatalogUnavailable,
    }
}

impl From<ProvisionerWorkerError> for WorkspaceProvisioningError {
    fn from(error: ProvisionerWorkerError) -> Self {
        match error {
            ProvisionerWorkerError::Unauthorized => Self::ProvisionerWorkerUnauthorized,
            ProvisionerWorkerError::Conflict => Self::ProvisionerWorkerConflict,
            ProvisionerWorkerError::Unreachable => Self::ProvisionerWorkerUnavailable,
            ProvisionerWorkerError::InvalidPayload => Self::ProvisionerWorkerResponseInvalid,
            ProvisionerWorkerError::Failed => Self::ProvisionerWorkerFailed,
            ProvisionerWorkerError::AssetDownloadFailed
            | ProvisionerWorkerError::AssetAuthRequired
            | ProvisionerWorkerError::PathValidationFailed
            | ProvisionerWorkerError::StepTimeout
            | ProvisionerWorkerError::UnexpectedError => Self::ProvisionerWorkerFailed,
        }
    }
}

impl From<ProvisionerWorkerHttpGatewayInitError> for WorkspaceProvisioningError {
    fn from(_error: ProvisionerWorkerHttpGatewayInitError) -> Self {
        Self::ProvisionerWorkerUnavailable
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceProvisioningResult {
    pub workspace: Workspace,
    pub progress: WorkspaceProvisioningProgress,
}

pub(crate) fn result(workspace: Workspace) -> WorkspaceProvisioningResult {
    let progress = progress_for_workspace(&workspace);
    WorkspaceProvisioningResult {
        workspace,
        progress,
    }
}

pub(crate) fn result_with_progress(
    workspace: Workspace,
    progress: WorkspaceProvisioningProgress,
) -> WorkspaceProvisioningResult {
    WorkspaceProvisioningResult {
        workspace,
        progress,
    }
}

pub(crate) fn progress_for_workspace(workspace: &Workspace) -> WorkspaceProvisioningProgress {
    match workspace.lifecycle_state {
        WorkspaceLifecycleState::Draft => WorkspaceProvisioningProgress {
            status: WorkspaceProvisioningStatus::Idle,
            phase: WorkspaceProvisioningPhase::NotStarted,
            percent: Some(PROGRESS_NOT_STARTED),
            failure: None,
        },
        WorkspaceLifecycleState::Provisioning => {
            let phase = progress_phase_for_provisioning_workspace(workspace);
            WorkspaceProvisioningProgress {
                status: WorkspaceProvisioningStatus::Running,
                percent: progress_percent_for_phase(&phase),
                phase,
                failure: None,
            }
        }
        WorkspaceLifecycleState::Ready => WorkspaceProvisioningProgress {
            status: WorkspaceProvisioningStatus::Completed,
            phase: WorkspaceProvisioningPhase::Completed,
            percent: Some(PROGRESS_COMPLETED),
            failure: None,
        },
        WorkspaceLifecycleState::Failed => WorkspaceProvisioningProgress {
            status: WorkspaceProvisioningStatus::Failed,
            phase: WorkspaceProvisioningPhase::Failed,
            percent: None,
            failure: Some(
                workspace
                    .last_provisioning_failure
                    .clone()
                    .unwrap_or_else(legacy_failure),
            ),
        },
    }
}

fn progress_phase_for_provisioning_workspace(workspace: &Workspace) -> WorkspaceProvisioningPhase {
    if !workspace
        .persistent_storage_volume_snapshot
        .as_ref()
        .is_some_and(|snapshot| snapshot.provider_resource_status == ProviderResourceStatus::Ready)
    {
        WorkspaceProvisioningPhase::CreatingVolume
    } else if workspace.environment_prepared_at.is_none() {
        WorkspaceProvisioningPhase::StartingProvisioningPod
    } else if workspace.active_provisioning_pod_snapshot.is_some()
        || !has_ready_matching_endpoint_template(workspace)
        || !workspace
            .serverless_endpoint_snapshot
            .as_ref()
            .is_some_and(|snapshot| {
                matches!(
                    snapshot.provider_resource_status,
                    ProviderResourceStatus::Ready | ProviderResourceStatus::Running
                )
            })
    {
        WorkspaceProvisioningPhase::CreatingEndpoint
    } else {
        WorkspaceProvisioningPhase::ValidatingReadiness
    }
}

fn progress_percent_for_phase(phase: &WorkspaceProvisioningPhase) -> Option<u8> {
    match phase {
        WorkspaceProvisioningPhase::NotStarted => Some(PROGRESS_NOT_STARTED),
        WorkspaceProvisioningPhase::CreatingVolume => Some(PROGRESS_CREATING_VOLUME),
        WorkspaceProvisioningPhase::StartingProvisioningPod => {
            Some(PROGRESS_STARTING_PROVISIONING_POD)
        }
        WorkspaceProvisioningPhase::PreparingEnvironment => {
            Some(PROGRESS_PREPARING_ENVIRONMENT_START)
        }
        WorkspaceProvisioningPhase::CreatingEndpoint => Some(PROGRESS_CREATING_ENDPOINT),
        WorkspaceProvisioningPhase::ValidatingReadiness => Some(PROGRESS_VALIDATING_READINESS),
        WorkspaceProvisioningPhase::Completed => Some(PROGRESS_COMPLETED),
        WorkspaceProvisioningPhase::CleaningUp | WorkspaceProvisioningPhase::Failed => None,
    }
}

fn scale_preparing_environment_progress(worker_percent: Option<u8>) -> u8 {
    let worker_percent = u16::from(worker_percent.unwrap_or(0));
    let range =
        u16::from(PROGRESS_PREPARING_ENVIRONMENT_END - PROGRESS_PREPARING_ENVIRONMENT_START);
    let scaled = u16::from(PROGRESS_PREPARING_ENVIRONMENT_START) + (worker_percent * range / 100);
    scaled as u8
}

pub(crate) fn worker_readiness_progress() -> WorkspaceProvisioningProgress {
    let phase = WorkspaceProvisioningPhase::StartingProvisioningPod;
    WorkspaceProvisioningProgress {
        status: WorkspaceProvisioningStatus::Running,
        percent: progress_percent_for_phase(&phase),
        phase,
        failure: None,
    }
}

pub(crate) fn progress_from_worker_status(
    status: &ProvisionerWorkerStatus,
) -> WorkspaceProvisioningProgress {
    let phase = workspace_phase_from_worker_status(status);
    WorkspaceProvisioningProgress {
        status: workspace_status_from_worker_status(status),
        percent: progress_percent_from_worker_status(status, &phase),
        phase,
        failure: None,
    }
}

fn workspace_status_from_worker_status(
    status: &ProvisionerWorkerStatus,
) -> WorkspaceProvisioningStatus {
    match status.status {
        ProvisionerWorkerJobStatus::Idle | ProvisionerWorkerJobStatus::Running => {
            WorkspaceProvisioningStatus::Running
        }
        ProvisionerWorkerJobStatus::Cancelling | ProvisionerWorkerJobStatus::Cancelled => {
            WorkspaceProvisioningStatus::Cancelling
        }
        ProvisionerWorkerJobStatus::Succeeded => WorkspaceProvisioningStatus::Running,
        ProvisionerWorkerJobStatus::Failed => WorkspaceProvisioningStatus::Failed,
    }
}

fn workspace_phase_from_worker_status(
    status: &ProvisionerWorkerStatus,
) -> WorkspaceProvisioningPhase {
    match status.phase {
        ProvisionerWorkerPhase::Idle
        | ProvisionerWorkerPhase::Starting
        | ProvisionerWorkerPhase::ResolvingWorkflow => {
            WorkspaceProvisioningPhase::StartingProvisioningPod
        }
        ProvisionerWorkerPhase::PreparingWorkspace
        | ProvisionerWorkerPhase::DownloadingAssets
        | ProvisionerWorkerPhase::ValidatingAssets => {
            WorkspaceProvisioningPhase::PreparingEnvironment
        }
        ProvisionerWorkerPhase::Completed => WorkspaceProvisioningPhase::CreatingEndpoint,
        ProvisionerWorkerPhase::Cancelled => WorkspaceProvisioningPhase::CleaningUp,
        ProvisionerWorkerPhase::Failed => WorkspaceProvisioningPhase::Failed,
    }
}

fn progress_percent_from_worker_status(
    status: &ProvisionerWorkerStatus,
    phase: &WorkspaceProvisioningPhase,
) -> Option<u8> {
    match phase {
        WorkspaceProvisioningPhase::PreparingEnvironment => Some(
            scale_preparing_environment_progress(status.progress_percent),
        ),
        _ => progress_percent_for_phase(phase),
    }
}

pub(crate) fn persistent_storage_volume_snapshot(
    workspace: &Workspace,
    observation: NetworkVolumeObservation,
) -> PersistentStorageVolumeSnapshot {
    PersistentStorageVolumeSnapshot {
        gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
        provider_resource_id: observation.provider_resource_id,
        provider_resource_status: observation.provider_resource_status,
        mount_path: observation.mount_path,
    }
}

pub(crate) fn observed_provisioning_pod_snapshot(
    workspace: &Workspace,
    previous: &ProvisioningPodSnapshot,
    observation: ProvisioningPodObservation,
) -> ProvisioningPodSnapshot {
    ProvisioningPodSnapshot {
        gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
        provider_resource_id: observation.provider_resource_id,
        provider_resource_status: observation.provider_resource_status,
        provisioner_status_url: observation
            .provisioner_status_url
            .unwrap_or_else(|| previous.provisioner_status_url.clone()),
    }
}

pub(crate) fn serverless_endpoint_snapshot(
    workspace: &Workspace,
    observation: ServerlessEndpointObservation,
) -> ServerlessEndpointSnapshot {
    ServerlessEndpointSnapshot {
        gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
        provider_resource_id: observation.provider_resource_id,
        provider_resource_status: observation.provider_resource_status,
        endpoint_invoke_url: observation.endpoint_invoke_url,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::workspace::ProviderResourceStatus;
    use crate::workspace_provisioning::test_support::{
        endpoint, pod, provisioning_workspace, ready_provisioning_workspace, template, volume,
        workspace,
    };

    fn failed_workspace() -> Workspace {
        Workspace {
            lifecycle_state: WorkspaceLifecycleState::Failed,
            ..workspace()
        }
    }

    fn worker_status(
        status: ProvisionerWorkerJobStatus,
        phase: ProvisionerWorkerPhase,
        progress_percent: Option<u8>,
    ) -> ProvisionerWorkerStatus {
        ProvisionerWorkerStatus {
            status,
            phase,
            progress_percent,
        }
    }

    #[test]
    fn progress_for_workspace_maps_lifecycle_terminal_states() {
        let draft = workspace();
        assert_eq!(
            progress_for_workspace(&draft),
            WorkspaceProvisioningProgress {
                status: WorkspaceProvisioningStatus::Idle,
                phase: WorkspaceProvisioningPhase::NotStarted,
                percent: Some(0),
                failure: None,
            }
        );

        let ready = Workspace {
            lifecycle_state: WorkspaceLifecycleState::Ready,
            ..ready_provisioning_workspace()
        };
        assert_eq!(
            progress_for_workspace(&ready),
            WorkspaceProvisioningProgress {
                status: WorkspaceProvisioningStatus::Completed,
                phase: WorkspaceProvisioningPhase::Completed,
                percent: Some(100),
                failure: None,
            }
        );

        let failed_progress = progress_for_workspace(&failed_workspace());
        assert_eq!(failed_progress.status, WorkspaceProvisioningStatus::Failed);
        assert_eq!(failed_progress.phase, WorkspaceProvisioningPhase::Failed);
        assert_eq!(failed_progress.percent, None);
        assert_eq!(
            failed_progress.failure.expect("legacy failure").code,
            crate::domain::workspace::WorkspaceProvisioningFailureCode::LegacyFailure
        );
    }

    #[test]
    fn progress_for_workspace_maps_provisioning_phases_from_snapshots() {
        let mut workspace = provisioning_workspace();
        let progress = progress_for_workspace(&workspace);
        assert_eq!(progress.phase, WorkspaceProvisioningPhase::CreatingVolume);
        assert_eq!(progress.percent, Some(0));

        workspace.persistent_storage_volume_snapshot = Some(volume(ProviderResourceStatus::Ready));
        let progress = progress_for_workspace(&workspace);
        assert_eq!(
            progress.phase,
            WorkspaceProvisioningPhase::StartingProvisioningPod
        );
        assert_eq!(progress.percent, Some(10));

        workspace.active_provisioning_pod_snapshot = Some(pod(ProviderResourceStatus::Running));
        let progress = progress_for_workspace(&workspace);
        assert_eq!(
            progress.phase,
            WorkspaceProvisioningPhase::StartingProvisioningPod
        );
        assert_eq!(progress.percent, Some(10));

        workspace.environment_prepared_at = Some("2026-05-18T00:00:00Z".to_string());
        workspace.active_provisioning_pod_snapshot = None;
        let progress = progress_for_workspace(&workspace);
        assert_eq!(progress.phase, WorkspaceProvisioningPhase::CreatingEndpoint);
        assert_eq!(progress.percent, Some(90));

        workspace.provider_provisioning_snapshot = Some(
            crate::domain::workspace::ProviderProvisioningSnapshot::Runpod {
                endpoint_template_snapshot: Some(template(ProviderResourceStatus::Ready)),
            },
        );
        let progress = progress_for_workspace(&workspace);
        assert_eq!(progress.phase, WorkspaceProvisioningPhase::CreatingEndpoint);
        assert_eq!(progress.percent, Some(90));

        workspace.serverless_endpoint_snapshot = Some(endpoint(ProviderResourceStatus::Creating));
        let progress = progress_for_workspace(&workspace);
        assert_eq!(progress.phase, WorkspaceProvisioningPhase::CreatingEndpoint);
        assert_eq!(progress.percent, Some(90));

        workspace.serverless_endpoint_snapshot = Some(endpoint(ProviderResourceStatus::Ready));
        let progress = progress_for_workspace(&workspace);
        assert_eq!(
            progress.phase,
            WorkspaceProvisioningPhase::ValidatingReadiness
        );
        assert_eq!(progress.percent, Some(98));
    }

    #[test]
    fn progress_for_workspace_does_not_advance_on_present_but_unready_resources() {
        let mut workspace = provisioning_workspace();
        workspace.persistent_storage_volume_snapshot =
            Some(volume(ProviderResourceStatus::Creating));
        let progress = progress_for_workspace(&workspace);
        assert_eq!(progress.phase, WorkspaceProvisioningPhase::CreatingVolume);
        assert_eq!(progress.percent, Some(0));

        workspace.persistent_storage_volume_snapshot = Some(volume(ProviderResourceStatus::Ready));
        workspace.active_provisioning_pod_snapshot = Some(pod(ProviderResourceStatus::Creating));
        let progress = progress_for_workspace(&workspace);
        assert_eq!(
            progress.phase,
            WorkspaceProvisioningPhase::StartingProvisioningPod
        );
        assert_eq!(progress.percent, Some(10));

        workspace.environment_prepared_at = Some("2026-05-18T00:00:00Z".to_string());
        workspace.active_provisioning_pod_snapshot = None;
        workspace.provider_provisioning_snapshot = Some(
            crate::domain::workspace::ProviderProvisioningSnapshot::Runpod {
                endpoint_template_snapshot: Some(template(ProviderResourceStatus::Ready)),
            },
        );
        workspace.serverless_endpoint_snapshot = Some(endpoint(ProviderResourceStatus::Creating));
        let progress = progress_for_workspace(&workspace);
        assert_eq!(progress.phase, WorkspaceProvisioningPhase::CreatingEndpoint);
        assert_eq!(progress.percent, Some(90));
    }

    #[test]
    fn progress_for_workspace_keeps_running_pod_startup_until_worker_reports_preparation() {
        let mut workspace = provisioning_workspace();
        workspace.persistent_storage_volume_snapshot = Some(volume(ProviderResourceStatus::Ready));
        workspace.active_provisioning_pod_snapshot = Some(pod(ProviderResourceStatus::Running));

        let progress = progress_for_workspace(&workspace);
        assert_eq!(
            progress.phase,
            WorkspaceProvisioningPhase::StartingProvisioningPod
        );
        assert_eq!(progress.percent, Some(10));
    }

    #[test]
    fn progress_for_workspace_requires_ready_matching_endpoint_template_before_readiness() {
        let mut workspace = provisioning_workspace();
        workspace.persistent_storage_volume_snapshot = Some(volume(ProviderResourceStatus::Ready));
        workspace.environment_prepared_at = Some("2026-05-18T00:00:00Z".to_string());
        workspace.active_provisioning_pod_snapshot = None;
        workspace.serverless_endpoint_snapshot = Some(endpoint(ProviderResourceStatus::Ready));

        let progress = progress_for_workspace(&workspace);
        assert_eq!(progress.phase, WorkspaceProvisioningPhase::CreatingEndpoint);
        assert_eq!(progress.percent, Some(90));

        workspace.provider_provisioning_snapshot = Some(
            crate::domain::workspace::ProviderProvisioningSnapshot::Runpod {
                endpoint_template_snapshot: Some(template(ProviderResourceStatus::Creating)),
            },
        );
        let progress = progress_for_workspace(&workspace);
        assert_eq!(progress.phase, WorkspaceProvisioningPhase::CreatingEndpoint);
        assert_eq!(progress.percent, Some(90));

        workspace.provider_provisioning_snapshot = Some(
            crate::domain::workspace::ProviderProvisioningSnapshot::Runpod {
                endpoint_template_snapshot: Some(crate::domain::workspace::RunPodEndpointTemplateSnapshot {
                    endpoint_worker_image_ref: "ghcr.io/luma-forge/other@sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string(),
                    ..template(ProviderResourceStatus::Ready)
                }),
            },
        );
        let progress = progress_for_workspace(&workspace);
        assert_eq!(progress.phase, WorkspaceProvisioningPhase::CreatingEndpoint);
        assert_eq!(progress.percent, Some(90));
    }

    #[test]
    fn progress_from_worker_status_maps_worker_facts_to_workspace_progress() {
        let running = progress_from_worker_status(&worker_status(
            ProvisionerWorkerJobStatus::Running,
            ProvisionerWorkerPhase::DownloadingAssets,
            Some(55),
        ));
        assert_eq!(running.status, WorkspaceProvisioningStatus::Running);
        assert_eq!(
            running.phase,
            WorkspaceProvisioningPhase::PreparingEnvironment
        );
        assert_eq!(running.percent, Some(67));
        assert_eq!(running.failure, None);

        let startup = progress_from_worker_status(&worker_status(
            ProvisionerWorkerJobStatus::Running,
            ProvisionerWorkerPhase::Starting,
            Some(0),
        ));
        assert_eq!(startup.status, WorkspaceProvisioningStatus::Running);
        assert_eq!(
            startup.phase,
            WorkspaceProvisioningPhase::StartingProvisioningPod
        );
        assert_eq!(startup.percent, Some(10));

        let missing_preparation_percent = progress_from_worker_status(&worker_status(
            ProvisionerWorkerJobStatus::Running,
            ProvisionerWorkerPhase::DownloadingAssets,
            None,
        ));
        assert_eq!(
            missing_preparation_percent.phase,
            WorkspaceProvisioningPhase::PreparingEnvironment
        );
        assert_eq!(missing_preparation_percent.percent, Some(40));

        for (worker_percent, expected_percent) in [(0, 40), (50, 65), (100, 90)] {
            let progress = progress_from_worker_status(&worker_status(
                ProvisionerWorkerJobStatus::Running,
                ProvisionerWorkerPhase::DownloadingAssets,
                Some(worker_percent),
            ));
            assert_eq!(progress.percent, Some(expected_percent));
        }

        let cancelling = progress_from_worker_status(&worker_status(
            ProvisionerWorkerJobStatus::Cancelling,
            ProvisionerWorkerPhase::Cancelled,
            None,
        ));
        assert_eq!(cancelling.status, WorkspaceProvisioningStatus::Cancelling);
        assert_eq!(cancelling.phase, WorkspaceProvisioningPhase::CleaningUp);
        assert_eq!(cancelling.percent, None);

        let completed = progress_from_worker_status(&worker_status(
            ProvisionerWorkerJobStatus::Succeeded,
            ProvisionerWorkerPhase::Completed,
            Some(100),
        ));
        assert_eq!(completed.status, WorkspaceProvisioningStatus::Running);
        assert_eq!(
            completed.phase,
            WorkspaceProvisioningPhase::CreatingEndpoint
        );
        assert_eq!(completed.percent, Some(90));
    }

    #[test]
    fn worker_readiness_progress_is_starting_provisioning_pod_progress() {
        assert_eq!(
            worker_readiness_progress(),
            WorkspaceProvisioningProgress {
                status: WorkspaceProvisioningStatus::Running,
                phase: WorkspaceProvisioningPhase::StartingProvisioningPod,
                percent: Some(10),
                failure: None,
            }
        );
    }

    #[test]
    fn resource_catalog_errors_preserve_provisioning_categories() {
        for (resource_error, expected) in [
            (
                WorkspaceResourceError::WorkspaceCatalogUnavailable,
                WorkspaceProvisioningError::WorkspaceCatalogUnavailable,
            ),
            (
                WorkspaceResourceError::WorkspaceCatalogStorageUnavailable,
                WorkspaceProvisioningError::WorkspaceCatalogStorageUnavailable,
            ),
            (
                WorkspaceResourceError::WorkspaceCatalogMigrationFailed,
                WorkspaceProvisioningError::WorkspaceCatalogMigrationFailed,
            ),
            (
                WorkspaceResourceError::WorkspaceCatalogQueryFailed,
                WorkspaceProvisioningError::WorkspaceCatalogQueryFailed,
            ),
            (
                WorkspaceResourceError::WorkspaceCatalogCorrupt,
                WorkspaceProvisioningError::WorkspaceCatalogCorrupt,
            ),
            (
                WorkspaceResourceError::WorkspaceCatalogSchemaMismatch,
                WorkspaceProvisioningError::WorkspaceCatalogSchemaMismatch,
            ),
        ] {
            assert_eq!(WorkspaceProvisioningError::from(resource_error), expected);
        }
    }

    #[test]
    fn resource_command_errors_map_to_provisioning_categories() {
        for (resource_error, expected) in [
            (
                WorkspaceResourceError::ProviderSetupIncomplete,
                WorkspaceProvisioningError::ProviderSetupIncomplete,
            ),
            (
                WorkspaceResourceError::ProviderApiKeyUnauthorized,
                WorkspaceProvisioningError::ProviderApiKeyUnauthorized,
            ),
            (
                WorkspaceResourceError::ProviderApiUnavailable,
                WorkspaceProvisioningError::ProviderApiUnavailable,
            ),
            (
                WorkspaceResourceError::ProviderRateLimited,
                WorkspaceProvisioningError::ProviderRateLimited,
            ),
            (
                WorkspaceResourceError::ProviderRequestRejected,
                WorkspaceProvisioningError::ProviderRequestRejected,
            ),
            (
                WorkspaceResourceError::ProviderResponseInvalid,
                WorkspaceProvisioningError::ProviderResponseInvalid,
            ),
            (
                WorkspaceResourceError::ProviderResourceNotFound,
                WorkspaceProvisioningError::ProviderResourceNotFound,
            ),
            (
                WorkspaceResourceError::ProviderOperationConflict,
                WorkspaceProvisioningError::ProviderOperationConflict,
            ),
            (
                WorkspaceResourceError::ProviderOperationIndeterminate,
                WorkspaceProvisioningError::ProviderOperationIndeterminate,
            ),
            (
                WorkspaceResourceError::SecureKeyringUnavailable,
                WorkspaceProvisioningError::SecureKeyringUnavailable,
            ),
            (
                WorkspaceResourceError::ProvisionerWorkerTokenInvalid,
                WorkspaceProvisioningError::ProvisionerWorkerTokenInvalid,
            ),
        ] {
            assert_eq!(WorkspaceProvisioningError::from(resource_error), expected);
        }
    }

    #[test]
    fn provisioner_worker_gateway_initialization_error_maps_to_worker_unavailable() {
        assert_eq!(
            WorkspaceProvisioningError::from(ProvisionerWorkerHttpGatewayInitError),
            WorkspaceProvisioningError::ProvisionerWorkerUnavailable
        );
    }
}
