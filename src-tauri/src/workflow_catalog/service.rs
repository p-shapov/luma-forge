use crate::domain::workflow_preset::WorkflowPreset;

use super::{
    errors::WorkflowCatalogError,
    reader::{
        EndpointContractCatalogReader, ProvisionerContractCatalogReader, WorkflowCatalogReader,
    },
    validation::{validate_runtime_catalog, validate_workflows},
};

#[derive(Debug, Clone)]
pub struct WorkflowCatalogService<W, E, P> {
    workflow_reader: W,
    endpoint_contract_reader: E,
    provisioner_contract_reader: P,
}

impl<W, E, P> WorkflowCatalogService<W, E, P>
where
    W: WorkflowCatalogReader,
    E: EndpointContractCatalogReader,
    P: ProvisionerContractCatalogReader,
{
    pub fn new(
        workflow_reader: W,
        endpoint_contract_reader: E,
        provisioner_contract_reader: P,
    ) -> Self {
        Self {
            workflow_reader,
            endpoint_contract_reader,
            provisioner_contract_reader,
        }
    }

    pub fn get_workflows(&self) -> Result<Vec<WorkflowPreset>, WorkflowCatalogError> {
        let workflows = self.workflow_reader.read_workflows()?;
        let endpoint_contract_catalog = self
            .endpoint_contract_reader
            .read_endpoint_contract_catalog()?;
        let provisioner_contract_catalog = self
            .provisioner_contract_reader
            .read_provisioner_contract_catalog()?;

        validate_runtime_catalog(&endpoint_contract_catalog)?;
        validate_runtime_catalog(&provisioner_contract_catalog)?;
        validate_workflows(
            &workflows,
            &endpoint_contract_catalog,
            &provisioner_contract_catalog,
        )?;

        Ok(workflows)
    }

    pub fn get_workflow_by_id(
        &self,
        workflow_id: &str,
    ) -> Result<Option<WorkflowPreset>, WorkflowCatalogError> {
        Ok(self
            .get_workflows()?
            .into_iter()
            .find(|workflow| workflow.id == workflow_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        provider::GpuCloudProviderId,
        runtime_contract::{
            RuntimeCatalog, RuntimeContract, RuntimeContractReference, RuntimeContractRevision,
        },
        workflow_preset::{
            ModelAsset, ModelAssetSource, RemoteProviderRuntimeRequirements,
            RemoteRuntimeRequirements, WorkflowExecutionType,
        },
    };

    #[derive(Debug, Clone)]
    struct FakeReader {
        workflows: Vec<WorkflowPreset>,
        endpoint_contract_catalog: RuntimeCatalog,
        provisioner_contract_catalog: RuntimeCatalog,
    }

    impl WorkflowCatalogReader for FakeReader {
        fn read_workflows(&self) -> Result<Vec<WorkflowPreset>, WorkflowCatalogError> {
            Ok(self.workflows.clone())
        }
    }

    impl EndpointContractCatalogReader for FakeReader {
        fn read_endpoint_contract_catalog(&self) -> Result<RuntimeCatalog, WorkflowCatalogError> {
            Ok(self.endpoint_contract_catalog.clone())
        }
    }

    impl ProvisionerContractCatalogReader for FakeReader {
        fn read_provisioner_contract_catalog(
            &self,
        ) -> Result<RuntimeCatalog, WorkflowCatalogError> {
            Ok(self.provisioner_contract_catalog.clone())
        }
    }

    fn runtime_catalog(id: &str, version: &str) -> RuntimeCatalog {
        RuntimeCatalog {
            contracts: vec![RuntimeContract {
                id: id.to_string(),
                revisions: vec![RuntimeContractRevision {
                    version: version.to_string(),
                    image_ref: "ghcr.io/example/image@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_string(),
                }],
            }],
        }
    }

    fn valid_workflow(id: &str) -> WorkflowPreset {
        WorkflowPreset {
            id: id.to_string(),
            version: "1.0.0".to_string(),
            name: "ComfyUI HiDream O1 Dev".to_string(),
            execution_type: WorkflowExecutionType::T2i,
            requires_hugging_face_api_key: true,
            remote_runtime_requirements: RemoteRuntimeRequirements {
                required_base_volume_size_bytes: 18837849239,
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
            required_model_assets: vec![ModelAsset {
                id: "hidream-o1-image-dev-fp8-scaled".to_string(),
                name: "HiDream O1 Image Dev FP8 Scaled".to_string(),
                download_source: ModelAssetSource::Huggingface {
                    repository_id: "Comfy-Org/HiDream-O1-Image".to_string(),
                    file_path: "checkpoints/hidream_o1_image_dev_fp8_scaled.safetensors"
                        .to_string(),
                    revision: "e469681accde36057e32e4a3125e39929a1bcd68".to_string(),
                },
                install_comfyui_relative_path:
                    "models/checkpoints/hidream_o1_image_dev_fp8_scaled.safetensors".to_string(),
            }],
        }
    }

    fn service() -> WorkflowCatalogService<FakeReader, FakeReader, FakeReader> {
        let reader = FakeReader {
            workflows: vec![valid_workflow("comfyui-hidream-o1-dev")],
            endpoint_contract_catalog: runtime_catalog("comfyui-hidream-o1-dev", "1.0.15"),
            provisioner_contract_catalog: runtime_catalog("luma-forge-provisioner", "1.0.6"),
        };

        WorkflowCatalogService::new(reader.clone(), reader.clone(), reader)
    }

    fn service_with_workflows(
        workflows: Vec<WorkflowPreset>,
    ) -> WorkflowCatalogService<FakeReader, FakeReader, FakeReader> {
        service_with_catalogs(
            workflows,
            runtime_catalog("comfyui-hidream-o1-dev", "1.0.15"),
            runtime_catalog("luma-forge-provisioner", "1.0.6"),
        )
    }

    fn service_with_catalogs(
        workflows: Vec<WorkflowPreset>,
        endpoint_contract_catalog: RuntimeCatalog,
        provisioner_contract_catalog: RuntimeCatalog,
    ) -> WorkflowCatalogService<FakeReader, FakeReader, FakeReader> {
        let reader = FakeReader {
            workflows,
            endpoint_contract_catalog,
            provisioner_contract_catalog,
        };

        WorkflowCatalogService::new(reader.clone(), reader.clone(), reader)
    }

    #[test]
    fn get_workflows_returns_valid_workflows() {
        let workflows = service()
            .get_workflows()
            .expect("workflows should be valid");

        assert!(
            workflows
                .iter()
                .any(|workflow| workflow.id == "comfyui-hidream-o1-dev"),
            "expected bundled HiDream workflow"
        );
    }

    #[test]
    fn get_workflow_by_id_returns_matching_workflow() {
        let workflow = service()
            .get_workflow_by_id("comfyui-hidream-o1-dev")
            .expect("workflows should be valid")
            .expect("known workflow should be present");

        assert_eq!(workflow.id, "comfyui-hidream-o1-dev");
    }

    #[test]
    fn get_workflow_by_id_returns_none_for_unknown_workflow() {
        let workflow = service()
            .get_workflow_by_id("unknown-workflow")
            .expect("workflows should be valid");

        assert_eq!(workflow, None);
    }

    #[test]
    fn get_workflows_rejects_invalid_workflow_catalog() {
        let mut workflow = valid_workflow("comfyui-hidream-o1-dev");
        workflow.name = " ".to_string();

        assert_eq!(
            service_with_workflows(vec![workflow]).get_workflows(),
            Err(WorkflowCatalogError::ValidationFailed)
        );
    }

    #[test]
    fn get_workflows_rejects_invalid_endpoint_contract_catalog() {
        assert_eq!(
            service_with_catalogs(
                vec![valid_workflow("comfyui-hidream-o1-dev")],
                RuntimeCatalog { contracts: vec![] },
                runtime_catalog("luma-forge-provisioner", "1.0.6"),
            )
            .get_workflows(),
            Err(WorkflowCatalogError::ValidationFailed)
        );
    }

    #[test]
    fn get_workflows_rejects_invalid_provisioner_contract_catalog() {
        assert_eq!(
            service_with_catalogs(
                vec![valid_workflow("comfyui-hidream-o1-dev")],
                runtime_catalog("comfyui-hidream-o1-dev", "1.0.15"),
                RuntimeCatalog { contracts: vec![] },
            )
            .get_workflows(),
            Err(WorkflowCatalogError::ValidationFailed)
        );
    }
}
