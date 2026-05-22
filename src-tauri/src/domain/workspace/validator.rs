use std::collections::HashSet;

use crate::domain::{
    error::{DomainValidationError, DomainValidationResult},
    provider_setup::GpuCloudProviderId,
    provisioner::validator as provisioner_validator,
    runtime::validator as runtime_validator,
    validation::{is_blank, is_safe_absolute_posix_path},
};

use super::{
    PersistentStorageVolumeSnapshot, ProviderProvisioningSnapshot, ProviderResourceStatus,
    ProvisioningPodSnapshot, RunPodEndpointTemplateSnapshot, ServerlessEndpointSnapshot, Workspace,
    WorkspaceCatalog, WorkspaceLifecycleState,
};

pub fn validate_workspace_catalog(catalog: &WorkspaceCatalog) -> DomainValidationResult {
    let mut ids = HashSet::new();
    for workspace in &catalog.workspaces {
        if !ids.insert(workspace.id.as_str()) {
            return Err(DomainValidationError);
        }
        validate_workspace(workspace)?;
    }

    Ok(())
}

pub fn validate_workspace(workspace: &Workspace) -> DomainValidationResult {
    if is_blank(&workspace.id)
        || is_blank(&workspace.name)
        || workspace.placement_plan.gpu_cloud_provider_id() != workspace.gpu_cloud_provider_id
        || runtime_validator::validate_resolved_runtime_snapshot(&workspace.resolved_runtime_image)
            .is_err()
        || provisioner_validator::validate_resolved_provisioner_snapshot(
            &workspace.resolved_provisioner_image,
        )
        .is_err()
        || workspace
            .environment_prepared_at
            .as_deref()
            .is_some_and(is_blank)
    {
        return Err(DomainValidationError);
    }

    if matches!(workspace.lifecycle_state, WorkspaceLifecycleState::Draft)
        && (workspace.persistent_storage_volume_snapshot.is_some()
            || workspace.active_provisioning_pod_snapshot.is_some()
            || workspace.serverless_endpoint_snapshot.is_some()
            || workspace.last_provisioning_pod_snapshot.is_some()
            || workspace.provider_provisioning_snapshot.is_some()
            || workspace.environment_prepared_at.is_some()
            || workspace.last_provisioning_failure.is_some())
    {
        return Err(DomainValidationError);
    }

    if !matches!(workspace.lifecycle_state, WorkspaceLifecycleState::Failed)
        && workspace.last_provisioning_failure.is_some()
    {
        return Err(DomainValidationError);
    }

    if matches!(workspace.lifecycle_state, WorkspaceLifecycleState::Ready)
        && !has_ready_provisioning_state(workspace)
    {
        return Err(DomainValidationError);
    }

    if let Some(snapshot) = &workspace.persistent_storage_volume_snapshot {
        validate_persistent_storage_volume_snapshot(workspace.gpu_cloud_provider_id, snapshot)?;
    }
    if let Some(snapshot) = &workspace.active_provisioning_pod_snapshot {
        validate_provisioning_pod_snapshot(workspace.gpu_cloud_provider_id, snapshot)?;
    }
    if let Some(snapshot) = &workspace.serverless_endpoint_snapshot {
        validate_serverless_endpoint_snapshot(workspace.gpu_cloud_provider_id, snapshot)?;
    }
    if let Some(snapshot) = &workspace.last_provisioning_pod_snapshot {
        validate_provisioning_pod_snapshot(workspace.gpu_cloud_provider_id, snapshot)?;
    }
    if let Some(snapshot) = &workspace.provider_provisioning_snapshot {
        validate_provider_provisioning_snapshot(workspace.gpu_cloud_provider_id, snapshot)?;
    }
    if workspace.serverless_endpoint_snapshot.is_some()
        && runpod_endpoint_template_snapshot(workspace).is_none()
    {
        return Err(DomainValidationError);
    }

    Ok(())
}

fn has_ready_provisioning_state(workspace: &Workspace) -> bool {
    workspace.active_provisioning_pod_snapshot.is_none()
        && workspace.environment_prepared_at.is_some()
        && workspace
            .persistent_storage_volume_snapshot
            .as_ref()
            .is_some_and(|snapshot| {
                snapshot.provider_resource_status == ProviderResourceStatus::Ready
            })
        && runpod_endpoint_template_snapshot(workspace).is_some_and(|snapshot| {
            snapshot.provider_resource_status == ProviderResourceStatus::Ready
        })
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

fn runpod_endpoint_template_snapshot(
    workspace: &Workspace,
) -> Option<&RunPodEndpointTemplateSnapshot> {
    match &workspace.provider_provisioning_snapshot {
        Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot,
        }) => endpoint_template_snapshot.as_ref(),
        None => None,
    }
}

fn validate_persistent_storage_volume_snapshot(
    provider_id: GpuCloudProviderId,
    snapshot: &PersistentStorageVolumeSnapshot,
) -> DomainValidationResult {
    if snapshot.gpu_cloud_provider_id != provider_id
        || is_blank(&snapshot.provider_resource_id)
        || !is_safe_absolute_posix_path(&snapshot.mount_path)
    {
        return Err(DomainValidationError);
    }

    Ok(())
}

fn validate_provisioning_pod_snapshot(
    provider_id: GpuCloudProviderId,
    snapshot: &ProvisioningPodSnapshot,
) -> DomainValidationResult {
    if snapshot.gpu_cloud_provider_id != provider_id
        || is_blank(&snapshot.provider_resource_id)
        || is_blank(&snapshot.provisioner_status_url)
    {
        return Err(DomainValidationError);
    }

    Ok(())
}

fn validate_serverless_endpoint_snapshot(
    provider_id: GpuCloudProviderId,
    snapshot: &ServerlessEndpointSnapshot,
) -> DomainValidationResult {
    if snapshot.gpu_cloud_provider_id != provider_id
        || is_blank(&snapshot.provider_resource_id)
        || is_blank(&snapshot.endpoint_invoke_url)
    {
        return Err(DomainValidationError);
    }

    Ok(())
}

fn validate_provider_provisioning_snapshot(
    provider_id: GpuCloudProviderId,
    snapshot: &ProviderProvisioningSnapshot,
) -> DomainValidationResult {
    match (provider_id, snapshot) {
        (
            GpuCloudProviderId::Runpod,
            ProviderProvisioningSnapshot::Runpod {
                endpoint_template_snapshot,
            },
        ) => {
            if let Some(template_snapshot) = endpoint_template_snapshot {
                validate_runpod_endpoint_template_snapshot(template_snapshot)?;
            }
        }
    }

    Ok(())
}

fn validate_runpod_endpoint_template_snapshot(
    snapshot: &RunPodEndpointTemplateSnapshot,
) -> DomainValidationResult {
    if is_blank(&snapshot.template_id)
        || is_blank(&snapshot.endpoint_worker_image_ref)
        || !is_safe_absolute_posix_path(&snapshot.mount_path)
    {
        return Err(DomainValidationError);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        placement::PlacementPlan,
        provisioner::ResolvedProvisionerImageSnapshot,
        runtime::ResolvedRuntimeImageSnapshot,
        workflow::{
            ProvisionerContractReference, RuntimeContractReference, WorkflowExecutionType,
            WorkflowPreset,
        },
        workspace::{
            WorkspaceProvisioningFailure, WorkspaceProvisioningFailureCode,
            WorkspaceProvisioningFailureSource, WorkspaceProvisioningPhase,
            WorkspaceProvisioningRecoveryAction,
        },
    };

    const DIGEST_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const DIGEST_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

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
            provisioner_contract: ProvisionerContractReference {
                id: "luma-forge-provisioner".to_string(),
                version: "1.0.0".to_string(),
            },
            required_model_assets: vec![],
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
            endpoint_image_ref: format!("ghcr.io/luma-forge/endpoint@sha256:{DIGEST_B}"),
        }
    }

    fn provisioner_snapshot() -> ResolvedProvisionerImageSnapshot {
        ResolvedProvisionerImageSnapshot {
            contract_id: "luma-forge-provisioner".to_string(),
            contract_version: "1.0.0".to_string(),
            provisioner_worker_image_ref: format!(
                "ghcr.io/luma-forge/provisioner@sha256:{DIGEST_C}"
            ),
            volume_mount_path: "/workspace".to_string(),
        }
    }

    fn draft_workspace() -> Workspace {
        Workspace::new_draft(
            GpuCloudProviderId::Runpod,
            "workspace-id".to_string(),
            "Workspace".to_string(),
            placement_plan(),
            runtime_snapshot(),
            provisioner_snapshot(),
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

    fn failure() -> WorkspaceProvisioningFailure {
        WorkspaceProvisioningFailure {
            code: WorkspaceProvisioningFailureCode::ReadinessValidationFailed,
            phase: WorkspaceProvisioningPhase::ValidatingReadiness,
            source: WorkspaceProvisioningFailureSource::Native,
            recovery_action: WorkspaceProvisioningRecoveryAction::Retry,
        }
    }

    fn ready_workspace(endpoint_status: ProviderResourceStatus) -> Workspace {
        let mut workspace = draft_workspace();
        workspace.lifecycle_state = WorkspaceLifecycleState::Ready;
        workspace.persistent_storage_volume_snapshot = Some(volume(ProviderResourceStatus::Ready));
        workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot: Some(template(ProviderResourceStatus::Ready)),
        });
        workspace.serverless_endpoint_snapshot = Some(endpoint(endpoint_status));
        workspace.environment_prepared_at = Some("2026-05-18T00:00:00Z".to_string());
        workspace
    }

    #[test]
    fn validate_workspace_accepts_clean_draft_and_ready_with_running_endpoint() {
        assert_eq!(validate_workspace(&draft_workspace()), Ok(()));
        assert_eq!(
            validate_workspace(&ready_workspace(ProviderResourceStatus::Running)),
            Ok(())
        );
    }

    #[test]
    fn validate_workspace_rejects_invalid_metadata() {
        let invalid_workspaces = [
            Workspace {
                id: " ".to_string(),
                ..draft_workspace()
            },
            Workspace {
                name: " ".to_string(),
                ..draft_workspace()
            },
            Workspace {
                resolved_runtime_image: ResolvedRuntimeImageSnapshot {
                    contract_version: "1.0".to_string(),
                    ..runtime_snapshot()
                },
                ..draft_workspace()
            },
            Workspace {
                resolved_provisioner_image: ResolvedProvisionerImageSnapshot {
                    volume_mount_path: "workspace".to_string(),
                    ..provisioner_snapshot()
                },
                ..draft_workspace()
            },
            Workspace {
                lifecycle_state: WorkspaceLifecycleState::Provisioning,
                environment_prepared_at: Some(" ".to_string()),
                ..draft_workspace()
            },
        ];

        for workspace in invalid_workspaces {
            assert_eq!(validate_workspace(&workspace), Err(DomainValidationError));
        }
    }

    #[test]
    fn validate_workspace_rejects_draft_with_provisioning_state() {
        let invalid_workspaces = [
            Workspace {
                persistent_storage_volume_snapshot: Some(volume(ProviderResourceStatus::Ready)),
                ..draft_workspace()
            },
            Workspace {
                active_provisioning_pod_snapshot: Some(pod(ProviderResourceStatus::Running)),
                ..draft_workspace()
            },
            Workspace {
                serverless_endpoint_snapshot: Some(endpoint(ProviderResourceStatus::Ready)),
                ..draft_workspace()
            },
            Workspace {
                provider_provisioning_snapshot: Some(ProviderProvisioningSnapshot::Runpod {
                    endpoint_template_snapshot: Some(template(ProviderResourceStatus::Ready)),
                }),
                ..draft_workspace()
            },
            Workspace {
                environment_prepared_at: Some("2026-05-18T00:00:00Z".to_string()),
                ..draft_workspace()
            },
            Workspace {
                last_provisioning_failure: Some(failure()),
                ..draft_workspace()
            },
        ];

        for workspace in invalid_workspaces {
            assert_eq!(validate_workspace(&workspace), Err(DomainValidationError));
        }
    }

    #[test]
    fn validate_workspace_rejects_failure_data_outside_failed_lifecycle() {
        let workspace = Workspace {
            lifecycle_state: WorkspaceLifecycleState::Provisioning,
            last_provisioning_failure: Some(failure()),
            ..draft_workspace()
        };

        assert_eq!(validate_workspace(&workspace), Err(DomainValidationError));
    }

    #[test]
    fn validate_workspace_rejects_ready_without_complete_ready_state() {
        let invalid_workspaces = [
            Workspace {
                persistent_storage_volume_snapshot: None,
                ..ready_workspace(ProviderResourceStatus::Ready)
            },
            Workspace {
                active_provisioning_pod_snapshot: Some(pod(ProviderResourceStatus::Running)),
                ..ready_workspace(ProviderResourceStatus::Ready)
            },
            Workspace {
                persistent_storage_volume_snapshot: Some(volume(ProviderResourceStatus::Creating)),
                ..ready_workspace(ProviderResourceStatus::Ready)
            },
            Workspace {
                provider_provisioning_snapshot: Some(ProviderProvisioningSnapshot::Runpod {
                    endpoint_template_snapshot: Some(template(ProviderResourceStatus::Creating)),
                }),
                ..ready_workspace(ProviderResourceStatus::Ready)
            },
            Workspace {
                serverless_endpoint_snapshot: Some(endpoint(ProviderResourceStatus::Creating)),
                ..ready_workspace(ProviderResourceStatus::Ready)
            },
            Workspace {
                environment_prepared_at: None,
                ..ready_workspace(ProviderResourceStatus::Ready)
            },
        ];

        for workspace in invalid_workspaces {
            assert_eq!(validate_workspace(&workspace), Err(DomainValidationError));
        }
    }

    #[test]
    fn validate_workspace_rejects_endpoint_without_template_snapshot() {
        let mut workspace = draft_workspace();
        workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
        workspace.serverless_endpoint_snapshot = Some(endpoint(ProviderResourceStatus::Ready));
        workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot: None,
        });

        assert_eq!(validate_workspace(&workspace), Err(DomainValidationError));
    }

    #[test]
    fn validate_workspace_rejects_invalid_resource_snapshots() {
        let invalid_workspaces = [
            Workspace {
                lifecycle_state: WorkspaceLifecycleState::Provisioning,
                persistent_storage_volume_snapshot: Some(PersistentStorageVolumeSnapshot {
                    provider_resource_id: " ".to_string(),
                    ..volume(ProviderResourceStatus::Ready)
                }),
                ..draft_workspace()
            },
            Workspace {
                lifecycle_state: WorkspaceLifecycleState::Provisioning,
                persistent_storage_volume_snapshot: Some(PersistentStorageVolumeSnapshot {
                    mount_path: "../workspace".to_string(),
                    ..volume(ProviderResourceStatus::Ready)
                }),
                ..draft_workspace()
            },
            Workspace {
                lifecycle_state: WorkspaceLifecycleState::Provisioning,
                active_provisioning_pod_snapshot: Some(ProvisioningPodSnapshot {
                    provisioner_status_url: " ".to_string(),
                    ..pod(ProviderResourceStatus::Running)
                }),
                ..draft_workspace()
            },
            Workspace {
                lifecycle_state: WorkspaceLifecycleState::Provisioning,
                serverless_endpoint_snapshot: Some(ServerlessEndpointSnapshot {
                    endpoint_invoke_url: " ".to_string(),
                    ..endpoint(ProviderResourceStatus::Ready)
                }),
                provider_provisioning_snapshot: Some(ProviderProvisioningSnapshot::Runpod {
                    endpoint_template_snapshot: Some(template(ProviderResourceStatus::Ready)),
                }),
                ..draft_workspace()
            },
            Workspace {
                lifecycle_state: WorkspaceLifecycleState::Provisioning,
                provider_provisioning_snapshot: Some(ProviderProvisioningSnapshot::Runpod {
                    endpoint_template_snapshot: Some(RunPodEndpointTemplateSnapshot {
                        mount_path: "workspace".to_string(),
                        ..template(ProviderResourceStatus::Ready)
                    }),
                }),
                ..draft_workspace()
            },
        ];

        for workspace in invalid_workspaces {
            assert_eq!(validate_workspace(&workspace), Err(DomainValidationError));
        }
    }

    #[test]
    fn validate_workspace_catalog_rejects_duplicate_ids_or_invalid_workspace() {
        let valid_workspace = draft_workspace();
        assert_eq!(
            validate_workspace_catalog(&WorkspaceCatalog {
                workspaces: vec![valid_workspace.clone()]
            }),
            Ok(())
        );

        assert_eq!(
            validate_workspace_catalog(&WorkspaceCatalog {
                workspaces: vec![valid_workspace.clone(), valid_workspace.clone()]
            }),
            Err(DomainValidationError)
        );

        assert_eq!(
            validate_workspace_catalog(&WorkspaceCatalog {
                workspaces: vec![Workspace {
                    name: " ".to_string(),
                    ..valid_workspace
                }]
            }),
            Err(DomainValidationError)
        );
    }
}
