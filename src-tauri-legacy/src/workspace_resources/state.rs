use crate::{
    domain::workspace::{
        PersistentStorageVolumeSnapshot, ProvisioningPodSnapshot,
        ServerlessEndpointProviderMetadata, ServerlessEndpointSnapshot, Workspace,
    },
    workspace_resources::{
        NetworkVolumeObservation, ProvisioningPodObservation, ServerlessEndpointObservation,
    },
};

pub(crate) fn reset_after_resource_cleanup(workspace: &mut Workspace) {
    workspace.persistent_storage_volume_snapshot = None;
    workspace.active_provisioning_pod_snapshot = None;
    workspace.serverless_endpoint_snapshot = None;
    workspace.last_provisioning_pod_snapshot = None;
    workspace.environment_prepared_at = None;
}

pub(crate) fn persistent_storage_volume_snapshot(
    workspace: &Workspace,
    observation: NetworkVolumeObservation,
) -> PersistentStorageVolumeSnapshot {
    PersistentStorageVolumeSnapshot {
        gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
        provider_resource_id: observation.provider_resource_id,
        provider_resource_status: observation.provider_resource_status,
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
    template_id: String,
) -> ServerlessEndpointSnapshot {
    ServerlessEndpointSnapshot {
        gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
        provider_resource_id: observation.provider_resource_id,
        provider_resource_status: observation.provider_resource_status,
        endpoint_invoke_url: observation.endpoint_invoke_url,
        provider_metadata: Some(ServerlessEndpointProviderMetadata::Runpod { template_id }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        placement::PlacementPlan,
        provider_setup::GpuCloudProviderId,
        provisioner::ResolvedProvisionerImageSnapshot,
        runtime::ResolvedRuntimeImageSnapshot,
        workflow::{
            ProvisionerContractReference, RuntimeContractReference, WorkflowExecutionType,
            WorkflowPreset,
        },
        workspace::{
            PersistentStorageVolumeSnapshot, ProviderResourceStatus,
            ServerlessEndpointProviderMetadata, ServerlessEndpointSnapshot,
            WorkspaceLifecycleState, WorkspaceProvisioningFailure,
            WorkspaceProvisioningFailureCode, WorkspaceProvisioningFailureSource,
            WorkspaceProvisioningPhase, WorkspaceProvisioningRecoveryAction,
        },
    };

    #[test]
    fn reset_after_resource_cleanup_returns_workspace_to_clean_draft() {
        let mut workspace = workspace_with_resources();
        workspace.lifecycle_state = WorkspaceLifecycleState::Failed;
        workspace.last_provisioning_failure = Some(failure());

        reset_after_resource_cleanup(&mut workspace);

        assert_eq!(workspace.lifecycle_state, WorkspaceLifecycleState::Failed);
        assert_eq!(workspace.persistent_storage_volume_snapshot, None);
        assert_eq!(workspace.active_provisioning_pod_snapshot, None);
        assert_eq!(workspace.serverless_endpoint_snapshot, None);
        assert_eq!(workspace.last_provisioning_pod_snapshot, None);
        assert_eq!(workspace.environment_prepared_at, None);
        assert_eq!(workspace.last_provisioning_failure, Some(failure()));
    }

    fn workspace_with_resources() -> Workspace {
        let preset = WorkflowPreset {
            id: "preset-1".to_string(),
            version: "1.0.0".to_string(),
            name: "Preset".to_string(),
            workflow_execution_type: WorkflowExecutionType::T2i,
            required_base_volume_size_bytes: 1,
            requires_hugging_face_api_key: false,
            runtime_contract: RuntimeContractReference {
                id: "runtime".to_string(),
                version: "1.0.0".to_string(),
            },
            provisioner_contract: ProvisionerContractReference {
                id: "provisioner".to_string(),
                version: "1.0.0".to_string(),
            },
            required_model_assets: Vec::new(),
        };
        let placement_plan = PlacementPlan::Runpod {
            selected_datacenter_id: "dc-1".to_string(),
            selected_gpu_id: "gpu-1".to_string(),
            persistent_storage_volume_size_bytes: 1,
            endpoint_keep_alive_seconds: 5,
            selected_workflow_preset: preset,
        };
        let runtime = ResolvedRuntimeImageSnapshot {
            contract_id: "runtime".to_string(),
            contract_version: "1.0.0".to_string(),
            endpoint_image_ref: "endpoint:latest".to_string(),
        };
        let provisioner = ResolvedProvisionerImageSnapshot {
            contract_id: "provisioner".to_string(),
            contract_version: "1.0.0".to_string(),
            provisioner_worker_image_ref: "provisioner:latest".to_string(),
        };
        let mut workspace = Workspace::new_draft(
            GpuCloudProviderId::Runpod,
            "workspace-1".to_string(),
            "Workspace".to_string(),
            placement_plan,
            runtime,
            provisioner,
        )
        .expect("workspace should be valid");
        workspace.persistent_storage_volume_snapshot = Some(PersistentStorageVolumeSnapshot {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            provider_resource_id: "volume-1".to_string(),
            provider_resource_status: ProviderResourceStatus::Ready,
        });
        workspace.serverless_endpoint_snapshot = Some(ServerlessEndpointSnapshot {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            provider_resource_id: "endpoint-1".to_string(),
            provider_resource_status: ProviderResourceStatus::Ready,
            endpoint_invoke_url: "https://endpoint.example/run".to_string(),
            provider_metadata: Some(ServerlessEndpointProviderMetadata::Runpod {
                template_id: "template-1".to_string(),
            }),
        });
        workspace.environment_prepared_at = Some("2026-05-18T00:00:00Z".to_string());
        workspace
    }

    fn failure() -> WorkspaceProvisioningFailure {
        WorkspaceProvisioningFailure {
            code: WorkspaceProvisioningFailureCode::CancellationCleanupFailed,
            phase: WorkspaceProvisioningPhase::CleaningUp,
            source: WorkspaceProvisioningFailureSource::Native,
            recovery_action: WorkspaceProvisioningRecoveryAction::CleanupWorkspaceResources,
        }
    }
}
