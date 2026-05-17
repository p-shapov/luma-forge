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
    #[serde(default = "default_runtime_manifest_compatibility")]
    pub runtime_manifest_compatibility: RuntimeManifestCompatibility,
    #[serde(default = "default_workspace_overlay_policy")]
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
    #[serde(default = "default_image_runtime_root_path")]
    pub image_runtime_root_path: String,
    #[serde(default = "default_image_python_interpreter_path")]
    pub image_python_interpreter_path: String,
    #[serde(default = "default_image_comfyui_root_path")]
    pub image_comfyui_root_path: String,
    #[serde(default = "default_image_base_dependency_record_paths")]
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

fn default_runtime_manifest_compatibility() -> RuntimeManifestCompatibility {
    RuntimeManifestCompatibility {
        manifest_version: "1".to_string(),
    }
}

fn default_workspace_overlay_policy() -> WorkspaceOverlayPolicy {
    WorkspaceOverlayPolicy {
        python_overlay_path: ".luma-forge/python-overlay".to_string(),
        import_path_precedence: "overlay_first".to_string(),
        protected_package_names: vec![
            "torch".to_string(),
            "torchvision".to_string(),
            "torchaudio".to_string(),
        ],
        protected_package_prefixes: vec!["nvidia-".to_string()],
    }
}

fn default_image_runtime_root_path() -> String {
    "/opt/luma-forge/runtime".to_string()
}

fn default_image_python_interpreter_path() -> String {
    "/opt/luma-forge/runtime/.venv/bin/python".to_string()
}

fn default_image_comfyui_root_path() -> String {
    "/opt/luma-forge/runtime/ComfyUI".to_string()
}

fn default_image_base_dependency_record_paths() -> Vec<String> {
    vec![
        "base-runtime/pip-freeze.txt".to_string(),
        "base-runtime/install-report.json".to_string(),
    ]
}
