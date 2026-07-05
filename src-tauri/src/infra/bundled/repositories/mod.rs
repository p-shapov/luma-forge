pub mod execution_schemas;
pub mod runtime_contracts;
pub mod runtime_presets;
pub mod workflows;

pub use execution_schemas::ExecutionSchemaRepository;
pub use runtime_contracts::RuntimeContractRepository;
pub use runtime_presets::RuntimePresetRepository;
pub use workflows::WorkflowRepository;

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::infra::bundled::{
        models::ExecutionSchemaOutputType, Catalog, ExecutionSchemaRepository,
        RuntimeContractRepository, RuntimePresetRepository, WorkflowRepository,
    };

    #[test]
    fn repositories_list_and_find_catalog_entries() {
        let catalog = Catalog::load(Path::new(env!("CARGO_MANIFEST_DIR")).join("../new_bundled"))
            .expect("catalog should load");

        let workflows = WorkflowRepository::new(catalog.clone());
        let runtime_contracts = RuntimeContractRepository::new(catalog.clone());
        let runtime_presets = RuntimePresetRepository::new(catalog.clone());
        let execution_schemas = ExecutionSchemaRepository::new(catalog);

        assert!(!workflows.list().is_empty());
        assert_eq!(
            workflows
                .find("comfyui-hidream-o1-dev", "1.0.0")
                .expect("workflow should exist")
                .name,
            "ComfyUI HiDream O1 Dev"
        );

        assert!(!runtime_contracts.list().is_empty());
        assert_eq!(
            runtime_contracts
                .find("provisioner", "1.0.0")
                .expect("runtime contract should exist")
                .image_ref,
            "ghcr.io/p-shapov/luma-forge/provisioner-worker@sha256:8f09164389385499f59495f030ec3c79f84eb8c3d6de5adab09cf9246afd1cc6"
        );

        assert!(!runtime_presets.list().is_empty());
        assert_eq!(
            runtime_presets
                .find("comfyui-py312-cu126-torch291", "1.0.0")
                .expect("runtime preset should exist")
                .runtime
                .python_version,
            "3.12"
        );

        assert!(!execution_schemas.list().is_empty());
        assert_eq!(
            execution_schemas
                .find("text-to-image", "1.0.0")
                .expect("execution schema should exist")
                .outputs
                .output_type,
            ExecutionSchemaOutputType::ImageSet
        );
    }
}
