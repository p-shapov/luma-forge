use crate::{
    domain::{
        runtime_contract::RuntimeContractReference,
        workflow_preset::RemoteProviderRuntimeRequirements,
        workspace::{
            ProvisionedRemoteComputeProvisioningError, ProvisionedRemoteComputeWorkspace, Workspace,
        },
    },
    workflow_catalog::WorkflowCatalogService,
};

pub(crate) struct ProvisionedRemoteComputeContractResolver<'a> {
    workflow_catalog_service: &'a WorkflowCatalogService,
}

impl<'a> ProvisionedRemoteComputeContractResolver<'a> {
    pub(crate) fn new(workflow_catalog_service: &'a WorkflowCatalogService) -> Self {
        Self {
            workflow_catalog_service,
        }
    }

    pub(crate) fn provisioner_image_ref(
        &self,
        workspace: &Workspace,
        runtime: &ProvisionedRemoteComputeWorkspace,
    ) -> Result<String, ProvisionedRemoteComputeProvisioningError> {
        let contract = self.runtime_contract_reference(workspace, runtime, |requirements| {
            &requirements.provisioner_contract
        })?;
        let catalog = self
            .workflow_catalog_service
            .get_provisioner_contract_catalog()
            .map_err(|error| {
                ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                    message: format!("provisioner contract catalog is invalid: {error:?}"),
                }
            })?;
        let resolved = catalog.resolve(contract).ok_or_else(|| {
            ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                message: format!(
                    "provisioner contract is not bundled: {}@{}",
                    contract.id, contract.version
                ),
            }
        })?;

        Ok(resolved.image_ref)
    }

    pub(crate) fn endpoint_image_ref(
        &self,
        workspace: &Workspace,
        runtime: &ProvisionedRemoteComputeWorkspace,
    ) -> Result<String, ProvisionedRemoteComputeProvisioningError> {
        let contract = self.runtime_contract_reference(workspace, runtime, |requirements| {
            &requirements.endpoint_contract
        })?;
        let catalog = self
            .workflow_catalog_service
            .get_endpoint_contract_catalog()
            .map_err(|error| {
                ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                    message: format!("endpoint contract catalog is invalid: {error:?}"),
                }
            })?;
        let resolved = catalog.resolve(contract).ok_or_else(|| {
            ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                message: format!(
                    "endpoint contract is not bundled: {}@{}",
                    contract.id, contract.version
                ),
            }
        })?;

        Ok(resolved.image_ref)
    }

    fn runtime_contract_reference<'w>(
        &self,
        workspace: &'w Workspace,
        runtime: &ProvisionedRemoteComputeWorkspace,
        contract: impl FnOnce(&'w RemoteProviderRuntimeRequirements) -> &'w RuntimeContractReference,
    ) -> Result<&'w RuntimeContractReference, ProvisionedRemoteComputeProvisioningError> {
        let provider_requirements = workspace
            .workflow_preset
            .remote_runtime_requirements
            .resolve_provider_requirements(runtime.remote_placement.gpu_cloud_provider_id)
            .ok_or_else(
                || ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                    message: format!(
                        "workflow preset has no runtime requirements for provider {:?}",
                        runtime.remote_placement.gpu_cloud_provider_id
                    ),
                },
            )?;

        Ok(contract(provider_requirements))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        domain::{
            placement::{RemoteEndpointKeepAliveLimits, RemotePlacementPlan},
            provider::GpuCloudProviderId,
            runtime_contract::RuntimeContractReference,
            workflow_preset::{
                RemoteProviderRuntimeRequirements, RemoteRuntimeRequirements,
                WorkflowExecutionType, WorkflowPreset,
            },
            workspace::{
                ProvisionedRemoteComputeProvisioningState,
                ProvisionedRemoteComputeProvisioningStatus, ProvisionedRemoteComputeResources,
                ProvisionedRemoteComputeWorkspace, Workspace, WorkspaceRuntime,
            },
        },
        workflow_catalog::WorkflowCatalogService,
    };

    use super::ProvisionedRemoteComputeContractResolver;

    const EXPECTED_ENDPOINT_IMAGE_REF: &str =
        "ghcr.io/p-shapov/luma-forge/runpod-endpoint-worker@sha256:ac7b4ee14423f5e74f444a03c429dece830fc4f72b01847df18b2a5b960cdd1a";

    const EXPECTED_PROVISIONER_IMAGE_REF: &str =
        "ghcr.io/p-shapov/luma-forge/provisioner-worker@sha256:8e0d74276a36db8b0fae428b492e8fd080eea5311a7d153a0d60023c7e5a8295";

    fn workflow_preset() -> WorkflowPreset {
        WorkflowPreset {
            id: "preset".to_string(),
            version: "1.0.0".to_string(),
            name: "Preset".to_string(),
            execution_type: WorkflowExecutionType::T2i,
            requires_hugging_face_api_key: false,
            remote_runtime_requirements: RemoteRuntimeRequirements {
                required_base_volume_size_bytes: 1,
                provider_requirements: vec![RemoteProviderRuntimeRequirements {
                    gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
                    endpoint_contract: RuntimeContractReference {
                        id: "comfyui-hidream-o1-dev".to_string(),
                        version: "1.0.15".to_string(),
                    },
                    provisioner_contract: RuntimeContractReference {
                        id: "luma-forge-provisioner".to_string(),
                        version: "1.0.6".to_string(),
                    },
                }],
            },
            required_model_assets: vec![],
        }
    }

    fn placement_plan() -> RemotePlacementPlan {
        RemotePlacementPlan {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            datacenter_id: "dc".to_string(),
            gpu_id: "gpu".to_string(),
            volume_size_bytes: 1,
            keep_alive_limits: Some(RemoteEndpointKeepAliveLimits {
                default_seconds: 60,
                min_seconds: 30,
                max_seconds: 120,
            }),
        }
    }

    fn workspace() -> Workspace {
        Workspace {
            id: "workspace".to_string(),
            workflow_preset: workflow_preset(),
            runtime: WorkspaceRuntime::ProvisionedRemoteCompute(
                ProvisionedRemoteComputeWorkspace {
                    remote_placement: placement_plan(),
                    provisioning: ProvisionedRemoteComputeProvisioningState {
                        status: ProvisionedRemoteComputeProvisioningStatus::NotStarted,
                        percent: None,
                    },
                    resources: ProvisionedRemoteComputeResources {
                        volume: None,
                        provisioner: None,
                        endpoint: None,
                    },
                },
            ),
        }
    }

    #[test]
    fn resolve_provisioner_image_ref_returns_bundled_image_ref() {
        let workflow_catalog_service = WorkflowCatalogService::new();
        let resolver = ProvisionedRemoteComputeContractResolver::new(&workflow_catalog_service);
        let workspace = workspace();
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &workspace.runtime;

        let image_ref = resolver
            .provisioner_image_ref(&workspace, remote)
            .expect("provisioner image ref should resolve");

        assert_eq!(image_ref, EXPECTED_PROVISIONER_IMAGE_REF);
    }

    #[test]
    fn resolve_endpoint_image_ref_returns_bundled_image_ref() {
        let workflow_catalog_service = WorkflowCatalogService::new();
        let resolver = ProvisionedRemoteComputeContractResolver::new(&workflow_catalog_service);
        let workspace = workspace();
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &workspace.runtime;

        let image_ref = resolver
            .endpoint_image_ref(&workspace, remote)
            .expect("endpoint image ref should resolve");

        assert_eq!(image_ref, EXPECTED_ENDPOINT_IMAGE_REF);
    }
}
