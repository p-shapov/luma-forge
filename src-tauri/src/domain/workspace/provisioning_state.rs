use super::{
    ProviderProvisioningSnapshot, ProviderResourceStatus, RunPodEndpointTemplateSnapshot,
    Workspace, WorkspaceLifecycleState, WorkspaceProvisioningFailure,
    WorkspaceProvisioningFailureCode, WorkspaceProvisioningFailureSource,
    WorkspaceProvisioningPhase, WorkspaceProvisioningProgress, WorkspaceProvisioningRecoveryAction,
    WorkspaceProvisioningStatus,
};

pub(crate) fn fail_workspace(workspace: &mut Workspace, failure: WorkspaceProvisioningFailure) {
    workspace.lifecycle_state = WorkspaceLifecycleState::Failed;
    workspace.last_provisioning_failure = Some(failure);
}

pub(crate) fn reset_after_resource_cleanup(workspace: &mut Workspace) {
    workspace.lifecycle_state = WorkspaceLifecycleState::Draft;
    workspace.persistent_storage_volume_snapshot = None;
    workspace.active_provisioning_pod_snapshot = None;
    workspace.serverless_endpoint_snapshot = None;
    workspace.last_provisioning_pod_snapshot = None;
    workspace.provider_provisioning_snapshot = None;
    workspace.environment_prepared_at = None;
    workspace.last_provisioning_failure = None;
}

pub(crate) fn runpod_template_snapshot(
    workspace: &Workspace,
) -> Option<RunPodEndpointTemplateSnapshot> {
    match &workspace.provider_provisioning_snapshot {
        Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot,
        }) => endpoint_template_snapshot.clone(),
        None => None,
    }
}

pub(crate) fn endpoint_template_matches_workspace(
    template: &RunPodEndpointTemplateSnapshot,
    workspace: &Workspace,
) -> bool {
    template.provider_resource_status == ProviderResourceStatus::Ready
        && template.endpoint_worker_image_ref == workspace.resolved_runtime_image.endpoint_image_ref
        && template.mount_path
            == workspace
                .persistent_storage_volume_snapshot
                .as_ref()
                .map(|volume| volume.mount_path.clone())
                .unwrap_or_default()
}

pub(crate) fn is_workspace_ready(workspace: &Workspace) -> bool {
    workspace.environment_prepared_at.is_some()
        && workspace.active_provisioning_pod_snapshot.is_none()
        && workspace
            .persistent_storage_volume_snapshot
            .as_ref()
            .is_some_and(|snapshot| {
                snapshot.provider_resource_status == ProviderResourceStatus::Ready
            })
        && runpod_template_snapshot(workspace)
            .as_ref()
            .is_some_and(|snapshot| endpoint_template_matches_workspace(snapshot, workspace))
        && workspace
            .serverless_endpoint_snapshot
            .as_ref()
            .is_some_and(|snapshot| {
                matches!(
                    snapshot.provider_resource_status,
                    ProviderResourceStatus::Ready | ProviderResourceStatus::Running
                )
            })
}

pub(crate) fn is_terminal_provider_resource_status(status: &ProviderResourceStatus) -> bool {
    matches!(
        status,
        ProviderResourceStatus::Failed
            | ProviderResourceStatus::Terminated
            | ProviderResourceStatus::Unknown
    )
}

pub(crate) fn progress_for_workspace(workspace: &Workspace) -> WorkspaceProvisioningProgress {
    match workspace.lifecycle_state {
        WorkspaceLifecycleState::Draft => WorkspaceProvisioningProgress {
            status: WorkspaceProvisioningStatus::Idle,
            phase: WorkspaceProvisioningPhase::NotStarted,
            percent: Some(0),
            failure: None,
        },
        WorkspaceLifecycleState::Provisioning => WorkspaceProvisioningProgress {
            status: WorkspaceProvisioningStatus::Running,
            phase: if workspace.persistent_storage_volume_snapshot.is_none() {
                WorkspaceProvisioningPhase::CreatingVolume
            } else if workspace.active_provisioning_pod_snapshot.is_none()
                && workspace.environment_prepared_at.is_none()
            {
                WorkspaceProvisioningPhase::StartingProvisioningPod
            } else if workspace.environment_prepared_at.is_none()
                || workspace.active_provisioning_pod_snapshot.is_some()
            {
                WorkspaceProvisioningPhase::PreparingEnvironment
            } else if workspace.serverless_endpoint_snapshot.is_none() {
                WorkspaceProvisioningPhase::CreatingEndpoint
            } else {
                WorkspaceProvisioningPhase::ValidatingReadiness
            },
            percent: None,
            failure: None,
        },
        WorkspaceLifecycleState::Ready => WorkspaceProvisioningProgress {
            status: WorkspaceProvisioningStatus::Completed,
            phase: WorkspaceProvisioningPhase::Completed,
            percent: Some(100),
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

fn legacy_failure() -> WorkspaceProvisioningFailure {
    WorkspaceProvisioningFailure {
        code: WorkspaceProvisioningFailureCode::LegacyFailure,
        phase: WorkspaceProvisioningPhase::Failed,
        source: WorkspaceProvisioningFailureSource::Native,
        retryable: false,
        recovery_action: WorkspaceProvisioningRecoveryAction::InspectWorkspaceProvisioning,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        placement::PlacementPlan,
        provider_setup::GpuCloudProviderId,
        runtime::ResolvedRuntimeImageSnapshot,
        workflow::{RuntimeContractReference, WorkflowExecutionType, WorkflowPreset},
        workspace::{
            PersistentStorageVolumeSnapshot, ProvisioningPodSnapshot, ServerlessEndpointSnapshot,
        },
    };

    const DIGEST_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const DIGEST_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn workflow_preset() -> WorkflowPreset {
        WorkflowPreset {
            id: "comfyui-t2i-basic".to_string(),
            version: "1.0.0".to_string(),
            name: "ComfyUI Text to Image".to_string(),
            workflow_execution_type: WorkflowExecutionType::T2i,
            required_base_volume_size_bytes: 80 * 1024 * 1024 * 1024,
            runtime_contract: RuntimeContractReference {
                id: "comfyui-python312-cu121".to_string(),
                version: "1.0.0".to_string(),
            },
            required_model_assets: vec![],
            required_custom_nodes: vec![],
        }
    }

    fn placement_plan() -> PlacementPlan {
        PlacementPlan::Runpod {
            selected_datacenter_id: "EU-RO-1".to_string(),
            selected_gpu_id: "NVIDIA A40".to_string(),
            persistent_storage_volume_size_bytes: 80 * 1024 * 1024 * 1024,
            endpoint_keep_alive_seconds: 5,
            selected_workflow_preset: workflow_preset(),
        }
    }

    fn runtime_snapshot() -> ResolvedRuntimeImageSnapshot {
        ResolvedRuntimeImageSnapshot {
            contract_id: "comfyui-python312-cu121".to_string(),
            contract_version: "1.0.0".to_string(),
            provisioner_image_ref: format!("ghcr.io/luma-forge/provisioner@sha256:{DIGEST_A}"),
            endpoint_image_ref: format!("ghcr.io/luma-forge/endpoint@sha256:{DIGEST_B}"),
        }
    }

    fn draft_workspace() -> Workspace {
        Workspace::new_draft(
            GpuCloudProviderId::Runpod,
            "workspace-id".to_string(),
            "Workspace".to_string(),
            placement_plan(),
            runtime_snapshot(),
        )
        .expect("valid draft workspace")
    }

    fn volume(status: ProviderResourceStatus) -> PersistentStorageVolumeSnapshot {
        PersistentStorageVolumeSnapshot {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            provider_resource_id: "volume-id".to_string(),
            provider_resource_status: status,
            mount_path: "/workspace".to_string(),
        }
    }

    fn pod(status: ProviderResourceStatus) -> ProvisioningPodSnapshot {
        ProvisioningPodSnapshot {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            provider_resource_id: "pod-id".to_string(),
            provider_resource_status: status,
            provisioner_status_url: "https://worker.example/status".to_string(),
        }
    }

    fn endpoint(status: ProviderResourceStatus) -> ServerlessEndpointSnapshot {
        ServerlessEndpointSnapshot {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            provider_resource_id: "endpoint-id".to_string(),
            provider_resource_status: status,
            endpoint_invoke_url: "https://endpoint.example/run".to_string(),
        }
    }

    fn template(status: ProviderResourceStatus) -> RunPodEndpointTemplateSnapshot {
        RunPodEndpointTemplateSnapshot {
            template_id: "template-id".to_string(),
            provider_resource_status: status,
            endpoint_worker_image_ref: runtime_snapshot().endpoint_image_ref,
            mount_path: "/workspace".to_string(),
        }
    }

    fn ready_workspace_with_endpoint_status(status: ProviderResourceStatus) -> Workspace {
        let mut workspace = draft_workspace();
        workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
        workspace.persistent_storage_volume_snapshot = Some(volume(ProviderResourceStatus::Ready));
        workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot: Some(template(ProviderResourceStatus::Ready)),
        });
        workspace.serverless_endpoint_snapshot = Some(endpoint(status));
        workspace.environment_prepared_at = Some("2026-05-18T00:00:00Z".to_string());
        workspace
    }

    fn failure() -> WorkspaceProvisioningFailure {
        WorkspaceProvisioningFailure {
            code: WorkspaceProvisioningFailureCode::ReadinessValidationFailed,
            phase: WorkspaceProvisioningPhase::ValidatingReadiness,
            source: WorkspaceProvisioningFailureSource::Native,
            retryable: true,
            recovery_action: WorkspaceProvisioningRecoveryAction::Retry,
        }
    }

    #[test]
    fn fail_workspace_records_failure_and_failed_lifecycle() {
        let mut workspace = draft_workspace();
        let failure = failure();

        fail_workspace(&mut workspace, failure.clone());

        assert_eq!(workspace.lifecycle_state, WorkspaceLifecycleState::Failed);
        assert_eq!(workspace.last_provisioning_failure, Some(failure));
    }

    #[test]
    fn reset_after_resource_cleanup_returns_workspace_to_clean_draft() {
        let mut workspace = ready_workspace_with_endpoint_status(ProviderResourceStatus::Running);
        workspace.lifecycle_state = WorkspaceLifecycleState::Failed;
        workspace.active_provisioning_pod_snapshot = Some(pod(ProviderResourceStatus::Failed));
        workspace.last_provisioning_pod_snapshot = Some(pod(ProviderResourceStatus::Terminated));
        workspace.last_provisioning_failure = Some(failure());

        reset_after_resource_cleanup(&mut workspace);

        assert_eq!(workspace.lifecycle_state, WorkspaceLifecycleState::Draft);
        assert_eq!(workspace.persistent_storage_volume_snapshot, None);
        assert_eq!(workspace.active_provisioning_pod_snapshot, None);
        assert_eq!(workspace.serverless_endpoint_snapshot, None);
        assert_eq!(workspace.last_provisioning_pod_snapshot, None);
        assert_eq!(workspace.provider_provisioning_snapshot, None);
        assert_eq!(workspace.environment_prepared_at, None);
        assert_eq!(workspace.last_provisioning_failure, None);
    }

    #[test]
    fn is_workspace_ready_accepts_ready_or_running_serverless_endpoint() {
        for status in [
            ProviderResourceStatus::Ready,
            ProviderResourceStatus::Running,
        ] {
            let workspace = ready_workspace_with_endpoint_status(status);

            assert!(is_workspace_ready(&workspace));
        }
    }

    #[test]
    fn is_workspace_ready_requires_matching_ready_template() {
        let mut workspace = ready_workspace_with_endpoint_status(ProviderResourceStatus::Ready);
        workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot: Some(RunPodEndpointTemplateSnapshot {
                endpoint_worker_image_ref: "ghcr.io/luma-forge/other@sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string(),
                ..template(ProviderResourceStatus::Ready)
            }),
        });

        assert!(!is_workspace_ready(&workspace));
    }

    #[test]
    fn terminal_provider_resource_statuses_are_failed_terminated_or_unknown() {
        for status in [
            ProviderResourceStatus::Failed,
            ProviderResourceStatus::Terminated,
            ProviderResourceStatus::Unknown,
        ] {
            assert!(is_terminal_provider_resource_status(&status));
        }

        for status in [
            ProviderResourceStatus::Creating,
            ProviderResourceStatus::Running,
            ProviderResourceStatus::Ready,
        ] {
            assert!(!is_terminal_provider_resource_status(&status));
        }
    }

    #[test]
    fn progress_for_workspace_maps_lifecycle_terminal_states() {
        let draft = draft_workspace();
        assert_eq!(
            progress_for_workspace(&draft),
            WorkspaceProvisioningProgress {
                status: WorkspaceProvisioningStatus::Idle,
                phase: WorkspaceProvisioningPhase::NotStarted,
                percent: Some(0),
                failure: None,
            }
        );

        let mut ready = ready_workspace_with_endpoint_status(ProviderResourceStatus::Running);
        ready.lifecycle_state = WorkspaceLifecycleState::Ready;
        assert_eq!(
            progress_for_workspace(&ready),
            WorkspaceProvisioningProgress {
                status: WorkspaceProvisioningStatus::Completed,
                phase: WorkspaceProvisioningPhase::Completed,
                percent: Some(100),
                failure: None,
            }
        );

        let mut failed = draft_workspace();
        failed.lifecycle_state = WorkspaceLifecycleState::Failed;
        assert_eq!(
            progress_for_workspace(&failed)
                .failure
                .expect("legacy failure")
                .code,
            WorkspaceProvisioningFailureCode::LegacyFailure
        );
    }

    #[test]
    fn progress_for_workspace_maps_provisioning_phases_from_snapshots() {
        let mut workspace = draft_workspace();
        workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
        assert_eq!(
            progress_for_workspace(&workspace).phase,
            WorkspaceProvisioningPhase::CreatingVolume
        );

        workspace.persistent_storage_volume_snapshot = Some(volume(ProviderResourceStatus::Ready));
        assert_eq!(
            progress_for_workspace(&workspace).phase,
            WorkspaceProvisioningPhase::StartingProvisioningPod
        );

        workspace.active_provisioning_pod_snapshot = Some(pod(ProviderResourceStatus::Running));
        assert_eq!(
            progress_for_workspace(&workspace).phase,
            WorkspaceProvisioningPhase::PreparingEnvironment
        );

        workspace.environment_prepared_at = Some("2026-05-18T00:00:00Z".to_string());
        workspace.active_provisioning_pod_snapshot = None;
        assert_eq!(
            progress_for_workspace(&workspace).phase,
            WorkspaceProvisioningPhase::CreatingEndpoint
        );

        workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot: Some(template(ProviderResourceStatus::Ready)),
        });
        assert_eq!(
            progress_for_workspace(&workspace).phase,
            WorkspaceProvisioningPhase::CreatingEndpoint
        );

        workspace.serverless_endpoint_snapshot = Some(endpoint(ProviderResourceStatus::Creating));
        assert_eq!(
            progress_for_workspace(&workspace).phase,
            WorkspaceProvisioningPhase::ValidatingReadiness
        );
    }
}
