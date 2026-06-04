# Workflow Catalog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a scalable native backend workflow catalog service backed by bundled catalogs, with reusable reader traits and service-level validation.

**Architecture:** Add `src-tauri/src/workflow_catalog/` as one service module with four files: `mod.rs`, `reader.rs`, `validation.rs`, and `service.rs`. Readers own catalog source access, validation is shared inside the service module, and `WorkflowCatalogService` orchestrates read, validate, list, and lookup behavior without adding Tauri commands.

**Tech Stack:** Rust 2021, `serde`, `serde_json`, existing domain structs, native `cargo test/fmt/clippy`.

---

## File Structure

- Create `src-tauri/src/workflow_catalog/mod.rs`: public module exports plus `WorkflowCatalogError` and `WorkflowCatalogResult`.
- Create `src-tauri/src/workflow_catalog/reader.rs`: reader traits and bundled JSON reader implementations.
- Create `src-tauri/src/workflow_catalog/validation.rs`: shared catalog validators used by the service.
- Create `src-tauri/src/workflow_catalog/service.rs`: `WorkflowCatalogService` and service behavior tests.
- Modify `src-tauri/src/lib.rs`: expose `pub mod workflow_catalog;`.
- Modify `src-tauri/Cargo.toml`: add `serde_json`.

Do not add Tauri commands, generated TypeScript bindings, frontend code, backend/API readers, caching, or domain-level validators in this plan.

## Task 1: Module Shell And Error Boundary

**Files:**
- Create: `src-tauri/src/workflow_catalog/mod.rs`
- Create: `src-tauri/src/workflow_catalog/reader.rs`
- Create: `src-tauri/src/workflow_catalog/service.rs`
- Create: `src-tauri/src/workflow_catalog/validation.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`

- [ ] **Step 1: Add `serde_json`**

Modify `src-tauri/Cargo.toml` dependencies:

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
specta = "=2.0.0-rc.24"
specta-typescript = "0.0.11"
tauri = { version = "2", features = [] }
tauri-plugin-mcp-bridge = "0.11"
tauri-plugin-opener = "2"
tauri-specta = { version = "=2.0.0-rc.24", features = ["derive", "typescript"] }
```

- [ ] **Step 2: Add module exports and error type**

Create `src-tauri/src/workflow_catalog/mod.rs`:

```rust
pub mod reader;
pub mod service;

mod validation;

pub use reader::{
    BundledEndpointContractCatalogReader, BundledProvisionerContractCatalogReader,
    BundledWorkflowCatalogReader, EndpointContractCatalogReader, ProvisionerContractCatalogReader,
    WorkflowCatalogReader,
};
pub use service::WorkflowCatalogService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowCatalogError {
    ParseFailed,
    ValidationFailed,
}

pub type WorkflowCatalogResult<T> = Result<T, WorkflowCatalogError>;
```

Create the reader trait shell in `src-tauri/src/workflow_catalog/reader.rs`:

`src-tauri/src/workflow_catalog/reader.rs`

```rust
use crate::domain::{
    runtime_contract::RuntimeCatalog,
    workflow_preset::WorkflowPreset,
};

use super::WorkflowCatalogResult;

pub trait WorkflowCatalogReader {
    fn read_workflows(&self) -> WorkflowCatalogResult<Vec<WorkflowPreset>>;
}

pub trait EndpointContractCatalogReader {
    fn read_endpoint_contract_catalog(&self) -> WorkflowCatalogResult<RuntimeCatalog>;
}

pub trait ProvisionerContractCatalogReader {
    fn read_provisioner_contract_catalog(&self) -> WorkflowCatalogResult<RuntimeCatalog>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BundledWorkflowCatalogReader;

#[derive(Debug, Clone, Copy, Default)]
pub struct BundledEndpointContractCatalogReader;

#[derive(Debug, Clone, Copy, Default)]
pub struct BundledProvisionerContractCatalogReader;
```

Create the service type shell in `src-tauri/src/workflow_catalog/service.rs`:

`src-tauri/src/workflow_catalog/service.rs`

```rust
#[derive(Debug, Clone)]
pub struct WorkflowCatalogService<W, E, P> {
    workflow_reader: W,
    endpoint_contract_reader: E,
    provisioner_contract_reader: P,
}

impl<W, E, P> WorkflowCatalogService<W, E, P> {
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
}
```

Create an empty validation module in `src-tauri/src/workflow_catalog/validation.rs`:

`src-tauri/src/workflow_catalog/validation.rs`

```rust
```

- [ ] **Step 3: Register the module**

Modify the module declarations in `src-tauri/src/lib.rs`:

```rust
pub mod domain;
pub mod provider_api_key;
pub mod remote_workspace;
pub mod shared;
pub mod workflow_catalog;
```

- [ ] **Step 4: Run compiler to verify the module shell compiles**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: PASS. There are no workflow catalog behavior tests yet.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/lib.rs src-tauri/src/workflow_catalog
git commit -m "feat(workflow-catalog): add module boundary"
```

## Task 2: Reader Traits And Bundled Readers

**Files:**
- Modify: `src-tauri/src/workflow_catalog/reader.rs`

- [ ] **Step 1: Write bundled reader tests**

Replace `src-tauri/src/workflow_catalog/reader.rs` with this test-first skeleton:

```rust
use serde::Deserialize;

use crate::domain::{
    runtime_contract::RuntimeCatalog,
    workflow_preset::WorkflowPreset,
};

use super::{WorkflowCatalogError, WorkflowCatalogResult};

#[derive(Debug, Clone, Deserialize)]
struct WorkflowCatalogJson {
    workflow_presets: Vec<WorkflowPreset>,
}

pub trait WorkflowCatalogReader {
    fn read_workflows(&self) -> WorkflowCatalogResult<Vec<WorkflowPreset>>;
}

pub trait EndpointContractCatalogReader {
    fn read_endpoint_contract_catalog(&self) -> WorkflowCatalogResult<RuntimeCatalog>;
}

pub trait ProvisionerContractCatalogReader {
    fn read_provisioner_contract_catalog(&self) -> WorkflowCatalogResult<RuntimeCatalog>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BundledWorkflowCatalogReader;

#[derive(Debug, Clone, Copy, Default)]
pub struct BundledEndpointContractCatalogReader;

#[derive(Debug, Clone, Copy, Default)]
pub struct BundledProvisionerContractCatalogReader;

#[cfg(test)]
mod tests {
    use super::{
        BundledEndpointContractCatalogReader, BundledProvisionerContractCatalogReader,
        BundledWorkflowCatalogReader, EndpointContractCatalogReader, ProvisionerContractCatalogReader,
        WorkflowCatalogReader,
    };

    #[test]
    fn bundled_workflow_reader_deserializes_workflows() {
        let workflows = BundledWorkflowCatalogReader
            .read_workflows()
            .expect("bundled workflows should deserialize");

        assert!(
            workflows.iter().any(|workflow| workflow.id == "comfyui-hidream-o1-dev"),
            "expected bundled HiDream workflow"
        );
    }

    #[test]
    fn bundled_endpoint_contract_reader_deserializes_contracts() {
        let catalog = BundledEndpointContractCatalogReader
            .read_endpoint_contract_catalog()
            .expect("bundled endpoint contracts should deserialize");

        assert!(
            catalog
                .contracts
                .iter()
                .any(|contract| contract.id == "comfyui-hidream-o1-dev"),
            "expected bundled HiDream endpoint contract"
        );
    }

    #[test]
    fn bundled_provisioner_contract_reader_deserializes_contracts() {
        let catalog = BundledProvisionerContractCatalogReader
            .read_provisioner_contract_catalog()
            .expect("bundled provisioner contracts should deserialize");

        assert!(
            catalog
                .contracts
                .iter()
                .any(|contract| contract.id == "luma-forge-provisioner"),
            "expected bundled provisioner contract"
        );
    }
}
```

- [ ] **Step 2: Run reader tests to verify failure**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml workflow_catalog::reader -- --nocapture
```

Expected: FAIL because the bundled reader traits are not implemented.

- [ ] **Step 3: Implement bundled readers**

Add these constants and impl blocks above the test module in `src-tauri/src/workflow_catalog/reader.rs`:

```rust
const WORKFLOW_CATALOG_JSON: &str = include_str!("../../../bundled/workflow-catalog.json");
const ENDPOINT_CONTRACTS_JSON: &str = include_str!("../../../bundled/endpoint-contracts.json");
const PROVISIONER_CONTRACTS_JSON: &str =
    include_str!("../../../bundled/provisioner-contracts.json");

impl WorkflowCatalogReader for BundledWorkflowCatalogReader {
    fn read_workflows(&self) -> WorkflowCatalogResult<Vec<WorkflowPreset>> {
        let catalog: WorkflowCatalogJson =
            serde_json::from_str(WORKFLOW_CATALOG_JSON).map_err(|_| WorkflowCatalogError::ParseFailed)?;

        Ok(catalog.workflow_presets)
    }
}

impl EndpointContractCatalogReader for BundledEndpointContractCatalogReader {
    fn read_endpoint_contract_catalog(&self) -> WorkflowCatalogResult<RuntimeCatalog> {
        serde_json::from_str(ENDPOINT_CONTRACTS_JSON).map_err(|_| WorkflowCatalogError::ParseFailed)
    }
}

impl ProvisionerContractCatalogReader for BundledProvisionerContractCatalogReader {
    fn read_provisioner_contract_catalog(&self) -> WorkflowCatalogResult<RuntimeCatalog> {
        serde_json::from_str(PROVISIONER_CONTRACTS_JSON)
            .map_err(|_| WorkflowCatalogError::ParseFailed)
    }
}
```

After formatting, the long `serde_json::from_str` line should be split by `cargo fmt`.

- [ ] **Step 4: Run reader tests to verify pass**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml workflow_catalog::reader -- --nocapture
```

Expected: PASS for the three bundled reader tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/workflow_catalog/reader.rs
git commit -m "feat(workflow-catalog): add bundled readers"
```

## Task 3: Shared Catalog Validation

**Files:**
- Modify: `src-tauri/src/workflow_catalog/validation.rs`

- [ ] **Step 1: Add validation tests**

Replace `src-tauri/src/workflow_catalog/validation.rs` with:

```rust
use std::collections::HashSet;

use crate::domain::{
    runtime_contract::RuntimeCatalog,
    workflow_preset::{
        ModelAssetSource, WorkflowPreset,
    },
};

use super::{WorkflowCatalogError, WorkflowCatalogResult};

pub(super) fn validate_runtime_catalog(
    catalog: &RuntimeCatalog,
) -> WorkflowCatalogResult<()> {
    let _ = catalog;
    Err(WorkflowCatalogError::ValidationFailed)
}

pub(super) fn validate_workflows(
    workflows: &[WorkflowPreset],
    endpoint_contract_catalog: &RuntimeCatalog,
    provisioner_contract_catalog: &RuntimeCatalog,
) -> WorkflowCatalogResult<()> {
    let _ = (workflows, endpoint_contract_catalog, provisioner_contract_catalog);
    Err(WorkflowCatalogError::ValidationFailed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        provider::GpuCloudProviderId,
        runtime_contract::{
            RuntimeContract, RuntimeContractReference, RuntimeContractRevision,
        },
        workflow_preset::{
            ModelAsset, ModelAssetSource, RemoteProviderRuntimeRequirements,
            RemoteRuntimeRequirements, WorkflowExecutionType,
        },
    };

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

    fn valid_asset() -> ModelAsset {
        ModelAsset {
            id: "hidream-o1-image-dev-fp8-scaled".to_string(),
            name: "HiDream O1 Image Dev FP8 Scaled".to_string(),
            download_source: ModelAssetSource::Huggingface {
                repository_id: "Comfy-Org/HiDream-O1-Image".to_string(),
                file_path: "checkpoints/hidream_o1_image_dev_fp8_scaled.safetensors".to_string(),
                revision: "e469681accde36057e32e4a3125e39929a1bcd68".to_string(),
            },
            install_comfyui_relative_path:
                "models/checkpoints/hidream_o1_image_dev_fp8_scaled.safetensors".to_string(),
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
            required_model_assets: vec![valid_asset()],
        }
    }

    #[test]
    fn validate_runtime_catalog_accepts_valid_catalog() {
        assert_eq!(
            validate_runtime_catalog(&runtime_catalog("comfyui-hidream-o1-dev", "1.0.15")),
            Ok(())
        );
    }

    #[test]
    fn validate_runtime_catalog_rejects_empty_catalog() {
        assert_eq!(
            validate_runtime_catalog(&RuntimeCatalog { contracts: vec![] }),
            Err(WorkflowCatalogError::ValidationFailed)
        );
    }

    #[test]
    fn validate_runtime_catalog_rejects_duplicate_contract_ids() {
        let catalog = RuntimeCatalog {
            contracts: vec![
                RuntimeContract {
                    id: "duplicate".to_string(),
                    revisions: vec![RuntimeContractRevision {
                        version: "1.0.0".to_string(),
                        image_ref: "image-a".to_string(),
                    }],
                },
                RuntimeContract {
                    id: "duplicate".to_string(),
                    revisions: vec![RuntimeContractRevision {
                        version: "1.0.1".to_string(),
                        image_ref: "image-b".to_string(),
                    }],
                },
            ],
        };

        assert_eq!(
            validate_runtime_catalog(&catalog),
            Err(WorkflowCatalogError::ValidationFailed)
        );
    }

    #[test]
    fn validate_workflows_accepts_valid_workflow() {
        let workflows = vec![valid_workflow("comfyui-hidream-o1-dev")];

        assert_eq!(
            validate_workflows(
                &workflows,
                &runtime_catalog("comfyui-hidream-o1-dev", "1.0.15"),
                &runtime_catalog("luma-forge-provisioner", "1.0.6"),
            ),
            Ok(())
        );
    }

    #[test]
    fn validate_workflows_rejects_duplicate_workflow_ids() {
        let workflows = vec![
            valid_workflow("comfyui-hidream-o1-dev"),
            valid_workflow("comfyui-hidream-o1-dev"),
        ];

        assert_eq!(
            validate_workflows(
                &workflows,
                &runtime_catalog("comfyui-hidream-o1-dev", "1.0.15"),
                &runtime_catalog("luma-forge-provisioner", "1.0.6"),
            ),
            Err(WorkflowCatalogError::ValidationFailed)
        );
    }

    #[test]
    fn validate_workflows_rejects_missing_endpoint_contract_reference() {
        let workflows = vec![valid_workflow("comfyui-hidream-o1-dev")];

        assert_eq!(
            validate_workflows(
                &workflows,
                &runtime_catalog("different-endpoint", "1.0.15"),
                &runtime_catalog("luma-forge-provisioner", "1.0.6"),
            ),
            Err(WorkflowCatalogError::ValidationFailed)
        );
    }

    #[test]
    fn validate_workflows_rejects_missing_provisioner_contract_reference() {
        let workflows = vec![valid_workflow("comfyui-hidream-o1-dev")];

        assert_eq!(
            validate_workflows(
                &workflows,
                &runtime_catalog("comfyui-hidream-o1-dev", "1.0.15"),
                &runtime_catalog("different-provisioner", "1.0.6"),
            ),
            Err(WorkflowCatalogError::ValidationFailed)
        );
    }

    #[test]
    fn validate_workflows_rejects_invalid_model_asset_paths() {
        let mut workflow = valid_workflow("comfyui-hidream-o1-dev");
        workflow.required_model_assets[0].install_comfyui_relative_path =
            "../outside.safetensors".to_string();
        let workflows = vec![workflow];

        assert_eq!(
            validate_workflows(
                &workflows,
                &runtime_catalog("comfyui-hidream-o1-dev", "1.0.15"),
                &runtime_catalog("luma-forge-provisioner", "1.0.6"),
            ),
            Err(WorkflowCatalogError::ValidationFailed)
        );
    }
}
```

The initial implementation returns validation errors for all inputs. This makes the acceptance tests fail before the real validation logic is added.

- [ ] **Step 2: Run validation tests to verify failure**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml workflow_catalog::validation -- --nocapture
```

Expected: FAIL because valid catalog tests receive `Err(WorkflowCatalogError::ValidationFailed)` instead of `Ok(())`.

- [ ] **Step 3: Implement validation**

Replace the two initial validation functions and add helpers in `src-tauri/src/workflow_catalog/validation.rs`:

```rust
pub(super) fn validate_runtime_catalog(catalog: &RuntimeCatalog) -> WorkflowCatalogResult<()> {
    if catalog.contracts.is_empty() {
        return Err(WorkflowCatalogError::ValidationFailed);
    }

    let mut contract_ids = HashSet::new();

    for contract in &catalog.contracts {
        if is_blank(&contract.id) || !contract_ids.insert(contract.id.as_str()) {
            return Err(WorkflowCatalogError::ValidationFailed);
        }

        if contract.revisions.is_empty() {
            return Err(WorkflowCatalogError::ValidationFailed);
        }

        let mut revision_versions = HashSet::new();

        for revision in &contract.revisions {
            if is_blank(&revision.version)
                || is_blank(&revision.image_ref)
                || !revision_versions.insert(revision.version.as_str())
            {
                return Err(WorkflowCatalogError::ValidationFailed);
            }
        }
    }

    Ok(())
}

pub(super) fn validate_workflows(
    workflows: &[WorkflowPreset],
    endpoint_contract_catalog: &RuntimeCatalog,
    provisioner_contract_catalog: &RuntimeCatalog,
) -> WorkflowCatalogResult<()> {
    if workflows.is_empty() {
        return Err(WorkflowCatalogError::ValidationFailed);
    }

    let mut workflow_ids = HashSet::new();

    for workflow in workflows {
        if is_blank(&workflow.id)
            || !workflow_ids.insert(workflow.id.as_str())
            || is_blank(&workflow.version)
            || is_blank(&workflow.name)
            || workflow.remote_runtime_requirements.required_base_volume_size_bytes == 0
            || workflow
                .remote_runtime_requirements
                .provider_requirements
                .is_empty()
        {
            return Err(WorkflowCatalogError::ValidationFailed);
        }

        for requirement in &workflow.remote_runtime_requirements.provider_requirements {
            if endpoint_contract_catalog
                .resolve(&requirement.endpoint_contract)
                .is_none()
                || provisioner_contract_catalog
                    .resolve(&requirement.provisioner_contract)
                    .is_none()
            {
                return Err(WorkflowCatalogError::ValidationFailed);
            }
        }

        for asset in &workflow.required_model_assets {
            if is_blank(&asset.id)
                || is_blank(&asset.name)
                || is_blank(&asset.install_comfyui_relative_path)
                || !is_safe_relative_path(&asset.install_comfyui_relative_path)
                || !is_valid_model_asset_source(&asset.download_source)
            {
                return Err(WorkflowCatalogError::ValidationFailed);
            }
        }
    }

    Ok(())
}

fn is_valid_model_asset_source(source: &ModelAssetSource) -> bool {
    match source {
        ModelAssetSource::Huggingface {
            repository_id,
            file_path,
            revision,
        } => {
            is_huggingface_repository_id(repository_id)
                && is_safe_relative_path(file_path)
                && !is_blank(revision)
        }
    }
}

fn is_huggingface_repository_id(value: &str) -> bool {
    let value = value.trim();
    let segments: Vec<_> = value.split('/').collect();

    segments.len() == 2
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && *segment != "."
                && *segment != ".."
                && segment.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
                })
        })
}

fn is_blank(value: &str) -> bool {
    value.trim().is_empty()
}

fn is_safe_relative_path(value: &str) -> bool {
    let value = value.trim();

    !value.is_empty()
        && !value.starts_with('/')
        && !value.starts_with('\\')
        && !value.contains('\\')
        && value
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}
```

- [ ] **Step 4: Run validation tests to verify pass**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml workflow_catalog::validation -- --nocapture
```

Expected: PASS for validation tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/workflow_catalog/validation.rs
git commit -m "feat(workflow-catalog): add shared validation"
```

## Task 4: Workflow Catalog Service

**Files:**
- Modify: `src-tauri/src/workflow_catalog/service.rs`

- [ ] **Step 1: Add service tests and skeleton**

Replace `src-tauri/src/workflow_catalog/service.rs` with:

```rust
use crate::domain::{
    workflow_preset::WorkflowPreset,
};

use super::{
    reader::{EndpointContractCatalogReader, ProvisionerContractCatalogReader, WorkflowCatalogReader},
    validation::{validate_runtime_catalog, validate_workflows},
    WorkflowCatalogResult,
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

    pub fn get_workflows(&self) -> WorkflowCatalogResult<Vec<WorkflowPreset>> {
        Ok(Vec::new())
    }

    pub fn get_workflow_by_id(
        &self,
        workflow_id: &str,
    ) -> WorkflowCatalogResult<Option<WorkflowPreset>> {
        let _ = workflow_id;
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use crate::workflow_catalog::reader::{
        BundledEndpointContractCatalogReader, BundledProvisionerContractCatalogReader,
        BundledWorkflowCatalogReader,
    };

    use super::*;

    fn bundled_service() -> WorkflowCatalogService<
        BundledWorkflowCatalogReader,
        BundledEndpointContractCatalogReader,
        BundledProvisionerContractCatalogReader,
    > {
        WorkflowCatalogService::new(
            BundledWorkflowCatalogReader,
            BundledEndpointContractCatalogReader,
            BundledProvisionerContractCatalogReader,
        )
    }

    #[test]
    fn get_workflows_returns_bundled_workflows() {
        let workflows = bundled_service()
            .get_workflows()
            .expect("bundled workflows should be valid");

        assert!(
            workflows.iter().any(|workflow| workflow.id == "comfyui-hidream-o1-dev"),
            "expected bundled HiDream workflow"
        );
    }

    #[test]
    fn get_workflow_by_id_returns_matching_workflow() {
        let workflow = bundled_service()
            .get_workflow_by_id("comfyui-hidream-o1-dev")
            .expect("bundled workflows should be valid")
            .expect("known workflow should be present");

        assert_eq!(workflow.id, "comfyui-hidream-o1-dev");
    }

    #[test]
    fn get_workflow_by_id_returns_none_for_unknown_workflow() {
        let workflow = bundled_service()
            .get_workflow_by_id("unknown-workflow")
            .expect("bundled workflows should be valid");

        assert_eq!(workflow, None);
    }
}
```

The initial implementation returns empty successful results. This makes the list and known-id tests fail before real orchestration is added.

- [ ] **Step 2: Run service tests to verify failure**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml workflow_catalog::service -- --nocapture
```

Expected: FAIL because `get_workflows` returns an empty list and `get_workflow_by_id` returns `None` for the known workflow.

- [ ] **Step 3: Implement service behavior**

Replace the two service methods in `src-tauri/src/workflow_catalog/service.rs`:

```rust
    pub fn get_workflows(&self) -> WorkflowCatalogResult<Vec<WorkflowPreset>> {
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
    ) -> WorkflowCatalogResult<Option<WorkflowPreset>> {
        let workflow = self
            .get_workflows()?
            .into_iter()
            .find(|workflow| workflow.id == workflow_id);

        Ok(workflow)
    }
```

- [ ] **Step 4: Run service tests to verify pass**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml workflow_catalog::service -- --nocapture
```

Expected: PASS for service tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/workflow_catalog/service.rs
git commit -m "feat(workflow-catalog): add catalog service"
```

## Task 5: Full Verification

**Files:**
- Inspect: `src-tauri/src/workflow_catalog/mod.rs`
- Inspect: `src-tauri/src/workflow_catalog/reader.rs`
- Inspect: `src-tauri/src/workflow_catalog/validation.rs`
- Inspect: `src-tauri/src/workflow_catalog/service.rs`
- Inspect: `src-tauri/src/lib.rs`
- Inspect: `src-tauri/Cargo.toml`

- [ ] **Step 1: Run full native tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 2: Run format check**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: PASS. If it fails, run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
```

Then rerun the format check.

- [ ] **Step 3: Run clippy**

Run:

```bash
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: PASS.

- [ ] **Step 4: Confirm command contracts were not touched**

Run:

```bash
git diff -- src/generated/commands.ts src-tauri/src/lib.rs
```

Expected:

- no diff for `src/generated/commands.ts`
- `src-tauri/src/lib.rs` only adds `pub mod workflow_catalog;`
- no command handler, `collect_commands!`, or TypeScript binding change for workflow catalog

- [ ] **Step 5: Commit verification cleanup if needed**

If `cargo fmt` changed files, commit the formatting cleanup:

```bash
git add src-tauri/src/workflow_catalog src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore(workflow-catalog): format catalog module"
```

If `cargo fmt --check` passed without modifying files, skip this commit.
