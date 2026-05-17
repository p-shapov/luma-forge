use serde::{Deserialize, Serialize};

pub mod validator;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCatalog {
    pub id: String,
    pub version: String,
    pub runtime_contracts: Vec<RuntimeContract>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeContract {
    pub id: String,
    pub version: String,
    pub display_name: String,
    pub runtime_metadata: RuntimeMetadata,
    pub implementation_revisions: Vec<RuntimeImplementationRevision>,
    pub default_implementation_revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeMetadata {
    pub environment_kind: String,
    pub python_version: String,
    pub platform: String,
    pub comfyui_revision: String,
    pub runtime_manifest_compatibility: RuntimeManifestCompatibility,
    pub workspace_overlay_policy: WorkspaceOverlayPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeManifestCompatibility {
    pub manifest_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceOverlayPolicy {
    pub python_overlay_path: String,
    pub import_path_precedence: String,
    pub protected_package_names: Vec<String>,
    pub protected_package_prefixes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeImplementationRevision {
    pub revision: String,
    pub provisioner_image_ref: String,
    pub endpoint_image_ref: String,
    pub image_metadata: RuntimeImageMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeImageMetadata {
    pub image_runtime_root_path: String,
    pub image_python_interpreter_path: String,
    pub image_comfyui_root_path: String,
    pub image_base_dependency_record_paths: Vec<String>,
    pub provisioner_runtime_metadata_path: String,
    pub endpoint_runtime_contract_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeContractReference {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedRuntimeImplementationSnapshot {
    pub contract_id: String,
    pub contract_version: String,
    pub implementation_revision: String,
    pub provisioner_image_ref: String,
    pub endpoint_image_ref: String,
    pub runtime_metadata: RuntimeMetadata,
    pub image_metadata: RuntimeImageMetadata,
}

impl RuntimeCatalog {
    pub fn resolve_default(
        &self,
        reference: &RuntimeContractReference,
    ) -> Option<ResolvedRuntimeImplementationSnapshot> {
        let contract = self.runtime_contracts.iter().find(|contract| {
            contract.id == reference.id && contract.version == reference.version
        })?;
        let implementation = contract
            .implementation_revisions
            .iter()
            .find(|implementation| {
                implementation.revision == contract.default_implementation_revision
            })?;

        Some(ResolvedRuntimeImplementationSnapshot {
            contract_id: contract.id.clone(),
            contract_version: contract.version.clone(),
            implementation_revision: implementation.revision.clone(),
            provisioner_image_ref: implementation.provisioner_image_ref.clone(),
            endpoint_image_ref: implementation.endpoint_image_ref.clone(),
            runtime_metadata: contract.runtime_metadata.clone(),
            image_metadata: implementation.image_metadata.clone(),
        })
    }
}
