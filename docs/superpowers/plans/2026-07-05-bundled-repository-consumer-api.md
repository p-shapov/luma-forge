# Bundled Repository Consumer API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the pass-through bundled catalog wrapper with stable bundled consumer models and direct repository `list`/`get`/RunPod resolve APIs.

**Architecture:** `generated.rs` remains the typify/generated manifest layer, `models.rs` becomes the stable consumer DTO layer, and `repositories/*.rs` read `generated::BUNDLED_ASSETS` directly. Build-time validation stays in `validation.rs`; errors live in `errors.rs`.

**Tech Stack:** Rust 2021, serde, serde_json, thiserror, generated typify DTOs, Tauri native backend.

## Global Constraints

- Do not edit old bundled catalog specs or plans.
- Do not wire this API into `workflow_catalog`, `runtime_catalog`, app state, Tauri commands, provider services, worker tooling, frontend bindings, or application ports.
- Do not add compatibility shims for removed bundled APIs or old flat bundled JSON assets.
- Do not add traits for single concrete repositories.
- Do not add new repository tests.
- Repository lookup misses return `Ok(None)`.
- Repository parse and assembly failures return `BundledCatalogError::CorruptBundledAsset { path, message }`.
- Remove secret-like execution input validation from bundled validation; do not replace it with another catalog validation rule.
- Keep the runtime public error surface to `BundledCatalogError::CorruptBundledAsset { path, message }`.
- Existing baseline note: `cargo check --manifest-path src-tauri/Cargo.toml -q` currently fails on old `include_str!` paths in `src-tauri/src/runtime_catalog/bundled.rs` and `src-tauri/src/workflow_catalog/bundled.rs`. This plan does not fix those old application paths.

---

## File Structure

- Modify `src-tauri/build.rs`: include `errors.rs` for the build script after moving `BundledValidationError`.
- Modify `src-tauri/src/infra/bundled/errors.rs`: hold both public runtime and crate-private validation errors.
- Modify `src-tauri/src/infra/bundled/validation.rs`: import `BundledValidationError`; remove secret-like policy helpers, calls, and tests.
- Delete `src-tauri/src/infra/bundled/catalog.rs`: no wrapper around `generated::BUNDLED_ASSETS`.
- Create `src-tauri/src/infra/bundled/models.rs`: stable consumer DTOs.
- Modify `src-tauri/src/infra/bundled/mod.rs`: remove `catalog`, add `models`, export the stable models and error.
- Modify `src-tauri/src/infra/bundled/repositories/mod.rs`: add shared parse/corrupt helpers over `generated::BUNDLED_ASSETS`.
- Modify `src-tauri/src/infra/bundled/repositories/runtime_presets.rs`: return `BundledRuntimePreset` from direct manifest reads.
- Modify `src-tauri/src/infra/bundled/repositories/runtime_contracts.rs`: return `BundledRuntimeContract` from direct manifest reads.
- Modify `src-tauri/src/infra/bundled/repositories/execution_schemas.rs`: return `BundledExecutionSchema` from direct manifest reads.
- Modify `src-tauri/src/infra/bundled/repositories/workflows.rs`: assemble `BundledWorkflow` and `ResolvedRunpodWorkflow` from the five workflow files and sibling repositories.

---

### Task 1: Move Validation Errors And Remove Secret-Like Policy

**Files:**
- Modify: `src-tauri/build.rs`
- Modify: `src-tauri/src/infra/bundled/errors.rs`
- Modify: `src-tauri/src/infra/bundled/validation.rs`

**Interfaces:**
- Consumes: current `BundledValidationError` definition in `validation.rs`
- Produces: `crate::infra::bundled::errors::BundledValidationError` for native code and `errors::BundledValidationError` for `build.rs`

- [ ] **Step 1: Move `BundledValidationError` into `errors.rs`**

Replace `src-tauri/src/infra/bundled/errors.rs` with:

```rust
#[derive(Debug, thiserror::Error)]
pub enum BundledCatalogError {
    #[error("bundled asset is corrupt: {path}: {message}")]
    CorruptBundledAsset { path: String, message: String },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum BundledValidationError {
    #[error("{path}: {message}")]
    Invalid { path: String, message: String },
}
```

- [ ] **Step 2: Make `validation.rs` use the moved error**

At the top of `src-tauri/src/infra/bundled/validation.rs`, add this import after the existing `use std::...` block:

```rust
use super::errors::BundledValidationError;
```

Delete this enum from `validation.rs`:

```rust
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BundledValidationError {
    #[error("{path}: {message}")]
    Invalid { path: String, message: String },
}
```

- [ ] **Step 3: Make the build script include `errors.rs`**

In `src-tauri/build.rs`, change:

```rust
#[path = "src/infra/bundled/validation.rs"]
mod validation;
```

to:

```rust
#[path = "src/infra/bundled/errors.rs"]
mod errors;
#[path = "src/infra/bundled/validation.rs"]
mod validation;
```

- [ ] **Step 4: Remove secret-like validation code**

In `src-tauri/src/infra/bundled/validation.rs`, delete the whole `is_secret_like` helper:

```rust
#[allow(dead_code)]
pub fn is_secret_like(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    value.contains("secret")
        || value.contains("token")
        || value.contains("password")
        || value.contains("api_key")
        || value.contains("apikey")
        || value.contains("credential")
}
```

In `validate_cross_file_assets`, delete this call:

```rust
reject_secret_like_execution_inputs(asset)?;
```

Delete the whole `reject_secret_like_execution_inputs` function.

- [ ] **Step 5: Remove validation tests for secret-like policy**

Delete these tests from `src-tauri/src/infra/bundled/validation.rs`:

```rust
fn secret_like_rejects_credential_names()
```

and:

```rust
fn validation_rejects_secret_like_execution_schema_inputs()
```

- [ ] **Step 6: Run the focused validation check**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml infra::bundled::validation --lib
```

Expected if old flat catalog includes are still unfixed:

```text
error: couldn't read `src/runtime_catalog/../../../bundled/runtime-contracts.json`
error: couldn't read `src/workflow_catalog/../../../bundled/workflow-catalog.json`
error: couldn't read `src/workflow_catalog/../../../bundled/execution-schemas.json`
```

If those old includes have already been fixed on the branch, expected result is:

```text
test result: ok.
```

- [ ] **Step 7: Commit**

```bash
git add src-tauri/build.rs src-tauri/src/infra/bundled/errors.rs src-tauri/src/infra/bundled/validation.rs
git commit -m "fix(bundled): move validation error boundary"
```

---

### Task 2: Add Stable Consumer Models

**Files:**
- Create: `src-tauri/src/infra/bundled/models.rs`
- Modify: `src-tauri/src/infra/bundled/mod.rs`

**Interfaces:**
- Consumes: generated schema concepts from `src-tauri/src/infra/bundled/generated.rs`
- Produces: stable DTOs used by all bundled repositories:
  `BundledReference`, `BundledWorkflow`, `BundledModelAsset`,
  `BundledWorkflowContractRequirement`, `BundledWorkflowExecutionContract`,
  `BundledRuntimePreset`, `BundledRuntimeContract`, `BundledExecutionSchema`,
  `ResolvedRunpodWorkflow`

- [ ] **Step 1: Create `models.rs`**

Create `src-tauri/src/infra/bundled/models.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledReference {
    pub id: String,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BundledWorkflow {
    pub id: String,
    pub revision: String,
    pub name: String,
    pub runtime_preset: BundledReference,
    pub requires_hugging_face_api_key: bool,
    pub required_volume_size_gb: u64,
    pub model_assets: Vec<BundledModelAsset>,
    pub contract_requirements: Vec<BundledWorkflowContractRequirement>,
    pub execution_contract: BundledWorkflowExecutionContract,
    pub graph: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledModelAsset {
    pub id: String,
    pub name: String,
    pub download_source: BundledModelAssetDownloadSource,
    pub install_comfyui_relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledModelAssetDownloadSource {
    pub source_type: String,
    pub repository_id: String,
    pub file_path: String,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledWorkflowContractRequirement {
    pub runtime_type: String,
    pub endpoint_contract: BundledReference,
    pub provisioner_contract: BundledReference,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BundledWorkflowExecutionContract {
    pub schema_ref: BundledReference,
    pub input_bindings: Vec<BundledWorkflowInputBinding>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BundledWorkflowInputBinding {
    pub value: serde_json::Value,
    pub node_id: String,
    pub path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledRuntimePreset {
    pub id: String,
    pub revision: String,
    pub runtime: BundledRuntimePresetRuntime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledRuntimePresetRuntime {
    pub python_version: String,
    pub comfyui_revision: String,
    pub pytorch: BundledRuntimePresetPytorch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledRuntimePresetPytorch {
    pub index_url: String,
    pub packages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledRuntimeContract {
    pub id: String,
    pub revision: String,
    pub image_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledExecutionSchema {
    pub id: String,
    pub revision: String,
    pub inputs: Vec<BundledExecutionInput>,
    pub output_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledExecutionInput {
    pub id: String,
    pub input_type: String,
    pub required: bool,
    pub max_length: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedRunpodWorkflow {
    pub workflow: BundledWorkflow,
    pub runtime_preset: BundledRuntimePreset,
    pub execution_schema: BundledExecutionSchema,
    pub endpoint_contract: BundledRuntimeContract,
    pub provisioner_contract: BundledRuntimeContract,
}
```

- [ ] **Step 2: Update bundled module exports**

In `src-tauri/src/infra/bundled/mod.rs`, change:

```rust
pub mod catalog;
pub mod errors;
pub mod generated;
pub mod repositories;
#[cfg(test)]
mod validation;

pub use catalog::BundledCatalog;
pub use errors::BundledCatalogError;
```

to:

```rust
pub mod errors;
pub mod generated;
pub mod models;
pub mod repositories;
#[cfg(test)]
mod validation;

pub use errors::BundledCatalogError;
pub use models::*;
```

- [ ] **Step 3: Run formatting**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected if the new file is formatted:

```text
```

`cargo fmt --check` prints no output on success.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/infra/bundled/mod.rs src-tauri/src/infra/bundled/models.rs
git commit -m "feat(bundled): add consumer models"
```

---

### Task 3: Refactor Repositories To Direct Generated Assets

**Files:**
- Delete: `src-tauri/src/infra/bundled/catalog.rs`
- Modify: `src-tauri/src/infra/bundled/repositories/mod.rs`
- Modify: `src-tauri/src/infra/bundled/repositories/runtime_presets.rs`
- Modify: `src-tauri/src/infra/bundled/repositories/runtime_contracts.rs`
- Modify: `src-tauri/src/infra/bundled/repositories/execution_schemas.rs`
- Modify: `src-tauri/src/infra/bundled/repositories/workflows.rs`

**Interfaces:**
- Consumes: `generated::BUNDLED_ASSETS`, generated top-level DTOs, and `models.rs`
- Produces:
  - `BundledRuntimePresetRepository::list/get`
  - `BundledRuntimeContractRepository::list/get`
  - `BundledExecutionSchemaRepository::list/get`
  - `BundledWorkflowRepository::list/get/resolve_runpod_workflow`

- [ ] **Step 1: Add shared repository helpers**

Replace `src-tauri/src/infra/bundled/repositories/mod.rs` with:

```rust
use serde::de::DeserializeOwned;

use super::{errors::BundledCatalogError, generated};

pub mod execution_schemas;
pub mod runtime_contracts;
pub mod runtime_presets;
pub mod workflows;

fn assets() -> &'static [(&'static str, &'static str)] {
    generated::BUNDLED_ASSETS
}

fn asset_text(path: &str) -> Option<&'static str> {
    assets()
        .iter()
        .find_map(|(asset_path, text)| (*asset_path == path).then_some(*text))
}

fn parse_asset<T: DeserializeOwned>(path: &str, text: &str) -> Result<T, BundledCatalogError> {
    serde_json::from_str(text).map_err(|error| corrupt(path, error.to_string()))
}

fn corrupt(path: &str, message: impl Into<String>) -> BundledCatalogError {
    BundledCatalogError::CorruptBundledAsset {
        path: path.to_string(),
        message: message.into(),
    }
}
```

- [ ] **Step 2: Refactor runtime presets repository**

Replace `src-tauri/src/infra/bundled/repositories/runtime_presets.rs` with a unit repository that:

```rust
use super::{asset_text, assets, parse_asset};
use crate::infra::bundled::{
    errors::BundledCatalogError,
    generated,
    models::{BundledRuntimePreset, BundledRuntimePresetPytorch, BundledRuntimePresetRuntime},
};

#[derive(Debug, Clone, Default)]
pub struct BundledRuntimePresetRepository;

impl BundledRuntimePresetRepository {
    pub fn new() -> Self {
        Self
    }

    pub fn list(&self) -> Result<Vec<BundledRuntimePreset>, BundledCatalogError> {
        assets()
            .iter()
            .filter(|(path, _)| path.starts_with("runtime_presets/"))
            .map(|(path, text)| parse_runtime_preset(path, text))
            .collect()
    }

    pub fn get(
        &self,
        id: &str,
        revision: &str,
    ) -> Result<Option<BundledRuntimePreset>, BundledCatalogError> {
        let path = format!("runtime_presets/{id}/{revision}.json");
        asset_text(&path)
            .map(|text| parse_runtime_preset(&path, text))
            .transpose()
    }
}

fn parse_runtime_preset(
    path: &str,
    text: &str,
) -> Result<BundledRuntimePreset, BundledCatalogError> {
    let preset = parse_asset::<generated::RuntimePreset>(path, text)?;
    Ok(BundledRuntimePreset {
        id: preset.id.into(),
        revision: preset.revision.into(),
        runtime: BundledRuntimePresetRuntime {
            python_version: preset.runtime.python_version.into(),
            comfyui_revision: preset.runtime.comfyui_revision.into(),
            pytorch: BundledRuntimePresetPytorch {
                index_url: preset.runtime.pytorch.index_url.into(),
                packages: preset
                    .runtime
                    .pytorch
                    .packages
                    .into_iter()
                    .map(Into::into)
                    .collect(),
            },
        },
    })
}
```

Delete the old `#[cfg(test)]` module in this file. The spec says not to add new repository tests.

- [ ] **Step 3: Refactor runtime contracts repository**

Replace `src-tauri/src/infra/bundled/repositories/runtime_contracts.rs` with the same direct pattern:

```rust
use super::{asset_text, assets, parse_asset};
use crate::infra::bundled::{
    errors::BundledCatalogError, generated, models::BundledRuntimeContract,
};

#[derive(Debug, Clone, Default)]
pub struct BundledRuntimeContractRepository;

impl BundledRuntimeContractRepository {
    pub fn new() -> Self {
        Self
    }

    pub fn list(&self) -> Result<Vec<BundledRuntimeContract>, BundledCatalogError> {
        assets()
            .iter()
            .filter(|(path, _)| path.starts_with("runtime_contracts/"))
            .map(|(path, text)| parse_runtime_contract(path, text))
            .collect()
    }

    pub fn get(
        &self,
        id: &str,
        revision: &str,
    ) -> Result<Option<BundledRuntimeContract>, BundledCatalogError> {
        let path = format!("runtime_contracts/{id}/{revision}.json");
        asset_text(&path)
            .map(|text| parse_runtime_contract(&path, text))
            .transpose()
    }
}

fn parse_runtime_contract(
    path: &str,
    text: &str,
) -> Result<BundledRuntimeContract, BundledCatalogError> {
    let contract = parse_asset::<generated::RuntimeContract>(path, text)?;
    Ok(BundledRuntimeContract {
        id: contract.id.into(),
        revision: contract.revision.into(),
        image_ref: contract.image_ref.into(),
    })
}
```

Delete the old `#[cfg(test)]` module in this file.

- [ ] **Step 4: Refactor execution schemas repository**

Replace `src-tauri/src/infra/bundled/repositories/execution_schemas.rs` with:

```rust
use super::{asset_text, assets, parse_asset};
use crate::infra::bundled::{
    errors::BundledCatalogError,
    generated,
    models::{BundledExecutionInput, BundledExecutionSchema},
};

#[derive(Debug, Clone, Default)]
pub struct BundledExecutionSchemaRepository;

impl BundledExecutionSchemaRepository {
    pub fn new() -> Self {
        Self
    }

    pub fn list(&self) -> Result<Vec<BundledExecutionSchema>, BundledCatalogError> {
        assets()
            .iter()
            .filter(|(path, _)| path.starts_with("execution_schemas/"))
            .map(|(path, text)| parse_execution_schema(path, text))
            .collect()
    }

    pub fn get(
        &self,
        id: &str,
        revision: &str,
    ) -> Result<Option<BundledExecutionSchema>, BundledCatalogError> {
        let path = format!("execution_schemas/{id}/{revision}.json");
        asset_text(&path)
            .map(|text| parse_execution_schema(&path, text))
            .transpose()
    }
}

fn parse_execution_schema(
    path: &str,
    text: &str,
) -> Result<BundledExecutionSchema, BundledCatalogError> {
    let schema = parse_asset::<generated::ExecutionSchema>(path, text)?;
    Ok(BundledExecutionSchema {
        id: schema.id.into(),
        revision: schema.revision.into(),
        inputs: schema
            .inputs
            .into_iter()
            .map(|input| BundledExecutionInput {
                id: input.id.into(),
                input_type: "string".to_string(),
                required: input.required,
                max_length: input.max_length.map(u64::from),
            })
            .collect(),
        output_type: schema.outputs.type_.into(),
    })
}
```

Delete the old `#[cfg(test)]` module in this file.

- [ ] **Step 5: Refactor workflow repository**

Replace `src-tauri/src/infra/bundled/repositories/workflows.rs` with a direct assembler. Use this structure:

```rust
use std::collections::BTreeSet;

use super::{
    asset_text, assets, corrupt, parse_asset,
    execution_schemas::BundledExecutionSchemaRepository,
    runtime_contracts::BundledRuntimeContractRepository,
    runtime_presets::BundledRuntimePresetRepository,
};
use crate::infra::bundled::{
    errors::BundledCatalogError,
    generated,
    models::{
        BundledModelAsset, BundledModelAssetDownloadSource, BundledReference, BundledWorkflow,
        BundledWorkflowContractRequirement, BundledWorkflowExecutionContract,
        BundledWorkflowInputBinding, ResolvedRunpodWorkflow,
    },
};

#[derive(Debug, Clone, Default)]
pub struct BundledWorkflowRepository;

impl BundledWorkflowRepository {
    pub fn new() -> Self {
        Self
    }

    pub fn list(&self) -> Result<Vec<BundledWorkflow>, BundledCatalogError> {
        workflow_revisions()
            .into_iter()
            .map(|(id, revision)| {
                self.get(&id, &revision)?
                    .ok_or_else(|| corrupt(&workflow_dir(&id, &revision), "workflow revision disappeared"))
            })
            .collect()
    }

    pub fn get(
        &self,
        id: &str,
        revision: &str,
    ) -> Result<Option<BundledWorkflow>, BundledCatalogError> {
        let metadata_path = workflow_file(id, revision, "metadata.json");
        let Some(metadata_text) = asset_text(&metadata_path) else {
            return Ok(None);
        };
        Ok(Some(parse_workflow(
            id,
            revision,
            &metadata_path,
            metadata_text,
        )?))
    }

    pub fn resolve_runpod_workflow(
        &self,
        id: &str,
        revision: &str,
        runtime_presets: &BundledRuntimePresetRepository,
        runtime_contracts: &BundledRuntimeContractRepository,
        execution_schemas: &BundledExecutionSchemaRepository,
    ) -> Result<Option<ResolvedRunpodWorkflow>, BundledCatalogError> {
        let Some(workflow) = self.get(id, revision)? else {
            return Ok(None);
        };
        let path = workflow_dir(id, revision);
        let runtime_preset = runtime_presets
            .get(&workflow.runtime_preset.id, &workflow.runtime_preset.revision)?
            .ok_or_else(|| corrupt(&path, "workflow runtime preset reference is missing"))?;
        let execution_schema = execution_schemas
            .get(
                &workflow.execution_contract.schema_ref.id,
                &workflow.execution_contract.schema_ref.revision,
            )?
            .ok_or_else(|| corrupt(&path, "workflow execution schema reference is missing"))?;
        let runpod = workflow
            .contract_requirements
            .iter()
            .find(|requirement| requirement.runtime_type == "runpod")
            .ok_or_else(|| corrupt(&path, "workflow has no RunPod contract requirement"))?;
        let endpoint_contract = runtime_contracts
            .get(&runpod.endpoint_contract.id, &runpod.endpoint_contract.revision)?
            .ok_or_else(|| corrupt(&path, "workflow endpoint contract reference is missing"))?;
        let provisioner_contract = runtime_contracts
            .get(
                &runpod.provisioner_contract.id,
                &runpod.provisioner_contract.revision,
            )?
            .ok_or_else(|| corrupt(&path, "workflow provisioner contract reference is missing"))?;

        Ok(Some(ResolvedRunpodWorkflow {
            workflow,
            runtime_preset,
            execution_schema,
            endpoint_contract,
            provisioner_contract,
        }))
    }
}
```

In the same file, add helper functions to assemble the model:

```rust
fn parse_workflow(
    id: &str,
    revision: &str,
    metadata_path: &str,
    metadata_text: &str,
) -> Result<BundledWorkflow, BundledCatalogError> {
    let metadata = parse_asset::<generated::WorkflowMetadata>(metadata_path, metadata_text)?;
    let model_assets = parse_asset::<generated::WorkflowModelAssets>(
        &workflow_file(id, revision, "model_assets.json"),
        required_text(id, revision, "model_assets.json")?,
    )?;
    let contract_requirements = parse_asset::<generated::WorkflowContractRequirements>(
        &workflow_file(id, revision, "contract_requirements.json"),
        required_text(id, revision, "contract_requirements.json")?,
    )?;
    let execution_contract = parse_asset::<generated::WorkflowExecutionContract>(
        &workflow_file(id, revision, "execution_contract.json"),
        required_text(id, revision, "execution_contract.json")?,
    )?;
    let graph = parse_asset::<generated::WorkflowGraph>(
        &workflow_file(id, revision, "workflow.json"),
        required_text(id, revision, "workflow.json")?,
    )?;

    Ok(BundledWorkflow {
        id: metadata.id.into(),
        revision: metadata.revision.into(),
        name: metadata.name.into(),
        runtime_preset: BundledReference {
            id: metadata.runtime_preset.id.into(),
            revision: metadata.runtime_preset.revision.into(),
        },
        requires_hugging_face_api_key: metadata.requires_hugging_face_api_key,
        required_volume_size_gb: metadata.required_volume_size_gb.get(),
        model_assets: model_assets
            .model_assets
            .into_iter()
            .map(|asset| BundledModelAsset {
                id: asset.id.into(),
                name: asset.name.into(),
                download_source: BundledModelAssetDownloadSource {
                    source_type: "huggingface".to_string(),
                    repository_id: asset.download_source.repository_id.into(),
                    file_path: asset.download_source.file_path.into(),
                    revision: asset.download_source.revision.into(),
                },
                install_comfyui_relative_path: asset.install_comfyui_relative_path.into(),
            })
            .collect(),
        contract_requirements: contract_requirements
            .contract_requirements
            .into_iter()
            .map(|requirement| BundledWorkflowContractRequirement {
                runtime_type: "runpod".to_string(),
                endpoint_contract: reference(requirement.endpoint_contract),
                provisioner_contract: reference(requirement.provisioner_contract),
            })
            .collect(),
        execution_contract: BundledWorkflowExecutionContract {
            schema_ref: BundledReference {
                id: execution_contract.schema_ref.id.into(),
                revision: execution_contract.schema_ref.revision.into(),
            },
            input_bindings: execution_contract
                .input_bindings
                .into_iter()
                .map(|binding| BundledWorkflowInputBinding {
                    value: binding.value,
                    node_id: binding.node_id.into(),
                    path: binding.path.into_iter().map(Into::into).collect(),
                })
                .collect(),
        },
        graph: serde_json::Value::Object(graph.graph),
    })
}

fn reference(reference: generated::Reference) -> BundledReference {
    BundledReference {
        id: reference.id.into(),
        revision: reference.revision.into(),
    }
}

fn required_text(
    id: &str,
    revision: &str,
    file: &str,
) -> Result<&'static str, BundledCatalogError> {
    let path = workflow_file(id, revision, file);
    asset_text(&path).ok_or_else(|| corrupt(&path, "required workflow file is missing"))
}

fn workflow_revisions() -> Vec<(String, String)> {
    assets()
        .iter()
        .filter_map(|(path, _)| {
            let parts: Vec<&str> = path.split('/').collect();
            match parts.as_slice() {
                ["workflows", id, revision, "metadata.json"] => {
                    Some(((*id).to_string(), (*revision).to_string()))
                }
                _ => None,
            }
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn workflow_file(id: &str, revision: &str, file: &str) -> String {
    format!("workflows/{id}/{revision}/{file}")
}

fn workflow_dir(id: &str, revision: &str) -> String {
    format!("workflows/{id}/{revision}")
}
```

Delete the old `workflow_revision_count`, `from_catalog`, and `#[cfg(test)]` module.

- [ ] **Step 6: Delete `catalog.rs`**

Delete:

```text
src-tauri/src/infra/bundled/catalog.rs
```

- [ ] **Step 7: Search for removed APIs**

Run:

```bash
rg -n "BundledCatalog|WorkflowRevisionPaths|from_catalog|from_assets|workflow_revision_count|catalog::" src-tauri/src/infra/bundled src-tauri/build.rs
```

Expected:

```text
```

No output means the old wrapper API is gone from `infra/bundled`.

- [ ] **Step 8: Run formatting**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected:

```text
```

If it prints diffs, run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
```

then rerun the `--check` command.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/infra/bundled
git commit -m "feat(bundled): expose consumer repository api"
```

---

### Task 4: Final Verification And Scope Check

**Files:**
- Check: `docs/superpowers/specs/2026-07-05-bundled-repository-consumer-api-design.md`
- Check: `src-tauri/src/infra/bundled/**`

**Interfaces:**
- Consumes: completed Tasks 1-3
- Produces: verified branch state or a precise note of the pre-existing old catalog include failure

- [ ] **Step 1: Confirm the old spec and plan were not modified**

Run:

```bash
git diff --name-only HEAD~3..HEAD | rg "2026-06-29-rust-side-bundled-catalogs-iteration|rewrite-bundled-catalogs-spec"
```

Expected:

```text
```

No output means this work did not edit the old bundled catalog spec/plan inputs.

- [ ] **Step 2: Confirm secret-like bundled validation is gone**

Run:

```bash
rg -n "secret_like|is_secret_like|reject_secret_like_execution_inputs|api_key|apikey|credential|password|token|secret" src-tauri/src/infra/bundled
```

Expected:

```text
```

No output means the bundled validation layer no longer enforces secret-like input naming.

- [ ] **Step 3: Confirm direct manifest reads**

Run:

```bash
rg -n "BUNDLED_ASSETS|asset_text|assets\\(" src-tauri/src/infra/bundled/repositories
```

Expected includes:

```text
src-tauri/src/infra/bundled/repositories/mod.rs:...generated::BUNDLED_ASSETS
```

- [ ] **Step 4: Run native checks**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Expected if old flat catalog includes are still unfixed:

```text
error: couldn't read `src/runtime_catalog/../../../bundled/runtime-contracts.json`
error: couldn't read `src/workflow_catalog/../../../bundled/workflow-catalog.json`
error: couldn't read `src/workflow_catalog/../../../bundled/execution-schemas.json`
```

Do not fix those old application path errors in this plan. Report them as pre-existing out-of-scope verification blockers.

If those old includes have already been fixed on the branch, expected result for all three commands is success.

- [ ] **Step 5: Commit verification-only formatting if needed**

If `cargo fmt` changed files, commit those exact formatting changes:

```bash
git add src-tauri/src/infra/bundled src-tauri/build.rs
git commit -m "style(bundled): format consumer repository api"
```

If `cargo fmt --check` already passed, do not create this commit.
