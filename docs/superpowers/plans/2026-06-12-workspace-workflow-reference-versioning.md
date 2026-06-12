# Workspace Workflow Reference Versioning Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Store only a versioned workflow reference on workspaces, move executable workflow fields into workflow revisions, and resolve `WorkflowPresetResolved` from `WorkflowCatalog`.

**Architecture:** `workflow_catalog` owns workflow preset metadata, revision validation, and `WorkflowCatalog::resolve(&WorkflowReference)`. `workspace_catalog` persists only `workflow_id` and `workflow_version` with runtime state. Command and lifecycle boundaries resolve references before they need executable workflow requirements.

**Tech Stack:** Rust, Tauri commands, serde, Specta, sqlx SQLite, bundled JSON catalogs, existing native test suite.

---

## File Structure

- Modify `src-tauri/src/domain/workflow_preset.rs`: add `WorkflowReference`, `WorkflowRevision`, `WorkflowPresetResolved`, and `WorkflowCatalog::resolve`.
- Modify `src-tauri/src/domain/workspace.rs`: replace embedded `WorkflowPreset` with `WorkflowReference`.
- Modify `src-tauri/src/workflow_catalog/validation.rs`: validate nested revisions and duplicate revision versions.
- Modify `bundled/workflow-catalog.json`: move executable fields under `revisions`.
- Modify `src-tauri/src/workspace_catalog/schema.rs`: replace `workflow_preset_json` with `workflow_id` and `workflow_version` without bumping `SCHEMA_VERSION`.
- Modify `src-tauri/src/workspace_catalog/sqlite.rs`: persist and read workflow references.
- Modify `src-tauri/src/workspace_catalog/service.rs`: update test helpers and repository-facing expectations.
- Modify `src-tauri/src/commands/types/catalog.rs`: expose revision-aware workflow catalog DTOs.
- Modify `src-tauri/src/commands/types/workspace.rs`: construct workspace responses from resolved workflows instead of direct `From<Workspace>`.
- Modify `src-tauri/src/commands/catalog.rs`: resolve workspace catalog responses through `WorkflowCatalogService`.
- Modify `src-tauri/src/commands/workspaces.rs`: accept revision version, resolve `WorkflowPresetResolved`, and persist `WorkflowReference`.
- Modify `src-tauri/src/provisioned_remote/service.rs`: create workspaces from `WorkflowPresetResolved` plus `WorkflowReference`.
- Modify `src-tauri/src/provisioned_remote/contracts.rs`: accept `WorkflowPresetResolved`.
- Modify `src-tauri/src/provisioned_remote/lifecycle/provision.rs`: resolve workflow before provisioning.
- Modify `src-tauri/src/app/events.rs`: resolve workflow references before emitting workspace events.
- Modify `src/generated/commands.ts`: regenerate with `bun run codegen:commands`.
- Modify `src/pages/home/ui/home-page.tsx`: add workflow revision selection to the diagnostics create-workspace sample request.

---

### Task 1: Add Workflow Reference and Resolved Workflow Domain Types

**Files:**

- Modify: `src-tauri/src/domain/workflow_preset.rs`

- [ ] **Step 1: Add failing tests for catalog resolution**

Add tests to `src-tauri/src/domain/workflow_preset.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        provisioned_remote::GpuCloudProviderId,
        runtime_contract::RuntimeContractReference,
    };

    fn reference(version: &str) -> WorkflowReference {
        WorkflowReference {
            id: "workflow".to_string(),
            version: version.to_string(),
        }
    }

    fn revision(version: &str, volume_size: u64) -> WorkflowRevision {
        WorkflowRevision {
            version: version.to_string(),
            requires_hugging_face_api_key: true,
            remote_runtime_requirements: RemoteRuntimeRequirements {
                required_base_volume_size_bytes: volume_size,
                provider_requirements: vec![RemoteProviderRuntimeRequirements {
                    gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
                    endpoint_contract: RuntimeContractReference {
                        id: "endpoint".to_string(),
                        version: "1.0.0".to_string(),
                    },
                    provisioner_contract: RuntimeContractReference {
                        id: "provisioner".to_string(),
                        version: "1.0.0".to_string(),
                    },
                }],
            },
            required_model_assets: Vec::new(),
        }
    }

    fn catalog() -> WorkflowCatalog {
        WorkflowCatalog {
            workflow_presets: vec![WorkflowPreset {
                id: "workflow".to_string(),
                name: "Workflow".to_string(),
                execution_type: WorkflowExecutionType::T2i,
                revisions: vec![revision("1.0.0", 1), revision("1.1.0", 2)],
            }],
        }
    }

    #[test]
    fn workflow_catalog_resolves_reference_to_revision() {
        let resolved = catalog()
            .resolve(&reference("1.1.0"))
            .expect("workflow reference should resolve");

        assert_eq!(resolved.id, "workflow");
        assert_eq!(resolved.version, "1.1.0");
        assert_eq!(resolved.name, "Workflow");
        assert_eq!(resolved.execution_type, WorkflowExecutionType::T2i);
        assert_eq!(resolved.remote_runtime_requirements.required_base_volume_size_bytes, 2);
    }

    #[test]
    fn workflow_catalog_rejects_missing_revision() {
        assert_eq!(catalog().resolve(&reference("2.0.0")), None);
    }
}
```

- [ ] **Step 2: Run the focused failing tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml domain::workflow_preset::tests::workflow_catalog_resolves_reference_to_revision domain::workflow_preset::tests::workflow_catalog_rejects_missing_revision
```

Expected: FAIL because `WorkflowReference`, `WorkflowRevision`, `WorkflowPresetResolved`, and `WorkflowCatalog::resolve` do not exist yet.

- [ ] **Step 3: Implement the domain model**

In `src-tauri/src/domain/workflow_preset.rs`, replace the flat preset with this shape:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowReference {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRevision {
    pub version: String,
    pub requires_hugging_face_api_key: bool,
    pub remote_runtime_requirements: RemoteRuntimeRequirements,
    pub required_model_assets: Vec<ModelAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowPreset {
    pub id: String,
    pub name: String,
    pub execution_type: WorkflowExecutionType,
    pub revisions: Vec<WorkflowRevision>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowPresetResolved {
    pub id: String,
    pub version: String,
    pub name: String,
    pub execution_type: WorkflowExecutionType,
    pub requires_hugging_face_api_key: bool,
    pub remote_runtime_requirements: RemoteRuntimeRequirements,
    pub required_model_assets: Vec<ModelAsset>,
}
```

Add the resolver:

```rust
impl WorkflowCatalog {
    pub fn resolve(&self, reference: &WorkflowReference) -> Option<WorkflowPresetResolved> {
        let preset = self
            .workflow_presets
            .iter()
            .find(|preset| preset.id == reference.id)?;
        let revision = preset
            .revisions
            .iter()
            .find(|revision| revision.version == reference.version)?;

        Some(WorkflowPresetResolved {
            id: preset.id.clone(),
            version: revision.version.clone(),
            name: preset.name.clone(),
            execution_type: preset.execution_type,
            requires_hugging_face_api_key: revision.requires_hugging_face_api_key,
            remote_runtime_requirements: revision.remote_runtime_requirements.clone(),
            required_model_assets: revision.required_model_assets.clone(),
        })
    }
}
```

- [ ] **Step 4: Run the focused tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml domain::workflow_preset::tests
```

Expected: PASS for the new resolver tests. Compilation may reveal old flat `WorkflowPreset` construction sites; leave those for later tasks.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/domain/workflow_preset.rs
git commit -m "feat(tauri): add resolved workflow preset model"
```

---

### Task 2: Update Workflow Catalog Validation and Bundled Catalog Shape

**Files:**

- Modify: `src-tauri/src/workflow_catalog/validation.rs`
- Modify: `src-tauri/src/workflow_catalog/service.rs`
- Modify: `bundled/workflow-catalog.json`

- [ ] **Step 1: Add failing validation tests**

In `src-tauri/src/workflow_catalog/validation.rs`, update the `valid_workflow` helper to construct a preset with `revisions: vec![valid_revision("1.0.0")]`, and add:

```rust
#[test]
fn validate_workflows_rejects_empty_workflow_revisions() {
    let mut workflow = valid_workflow("workflow");
    workflow.revisions.clear();

    assert_eq!(
        validate_workflows(
            &[workflow],
            &runtime_catalog("comfyui-py312-cu126-torch291", "1.0.15"),
            &runtime_catalog("luma-forge-provisioner", "1.0.6"),
        ),
        Err(WorkflowCatalogError::ValidationFailed)
    );
}

#[test]
fn validate_workflows_rejects_duplicate_revision_versions() {
    let mut workflow = valid_workflow("workflow");
    workflow.revisions.push(workflow.revisions[0].clone());

    assert_eq!(
        validate_workflows(
            &[workflow],
            &runtime_catalog("comfyui-py312-cu126-torch291", "1.0.15"),
            &runtime_catalog("luma-forge-provisioner", "1.0.6"),
        ),
        Err(WorkflowCatalogError::ValidationFailed)
    );
}
```

- [ ] **Step 2: Run validation tests and verify failure**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml workflow_catalog::validation::tests::validate_workflows_rejects_empty_workflow_revisions workflow_catalog::validation::tests::validate_workflows_rejects_duplicate_revision_versions
```

Expected: FAIL until validation walks `WorkflowPreset.revisions`.

- [ ] **Step 3: Implement revision validation**

In `validate_workflows`, keep preset validation to `id`, `name`, and preset-level fields. Replace flat executable validation with:

```rust
if workflow.revisions.is_empty() {
    return Err(WorkflowCatalogError::ValidationFailed);
}

let mut revision_versions = HashSet::new();
for revision in &workflow.revisions {
    if revision.version.trim().is_empty() || !revision_versions.insert(revision.version.as_str()) {
        return Err(WorkflowCatalogError::ValidationFailed);
    }

    let remote_requirements = &revision.remote_runtime_requirements;
    if remote_requirements.required_base_volume_size_bytes == 0
        || remote_requirements.provider_requirements.is_empty()
    {
        return Err(WorkflowCatalogError::ValidationFailed);
    }

    for provider_requirements in &remote_requirements.provider_requirements {
        if endpoint_contract_catalog
            .resolve(&provider_requirements.endpoint_contract)
            .is_none()
            || provisioner_contract_catalog
                .resolve(&provider_requirements.provisioner_contract)
                .is_none()
        {
            return Err(WorkflowCatalogError::ValidationFailed);
        }
    }

    for asset in &revision.required_model_assets {
        let install_path = asset.install_comfyui_relative_path.trim();
        if asset.id.trim().is_empty()
            || asset.name.trim().is_empty()
            || install_path.is_empty()
            || install_path.starts_with('/')
            || install_path.starts_with('\\')
            || install_path.contains('\\')
            || !install_path
                .split('/')
                .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
            || !is_valid_model_asset_source(&asset.download_source)
        {
            return Err(WorkflowCatalogError::ValidationFailed);
        }
    }
}
```

- [ ] **Step 4: Update bundled workflow catalog JSON**

Change `bundled/workflow-catalog.json` from flat executable fields to:

```json
{
  "workflow_presets": [
    {
      "id": "comfyui-hidream-o1-dev",
      "name": "ComfyUI HiDream O1 Dev",
      "execution_type": "t2i",
      "revisions": [
        {
          "version": "1.0.0",
          "requires_hugging_face_api_key": true,
          "remote_runtime_requirements": {
            "required_base_volume_size_bytes": 18837849239,
            "provider_requirements": [
              {
                "gpu_cloud_provider_id": "runpod",
                "endpoint_contract": {
                  "id": "comfyui-hidream-o1-dev",
                  "version": "1.0.15"
                },
                "provisioner_contract": {
                  "id": "luma-forge-provisioner",
                  "version": "1.0.7"
                }
              }
            ]
          },
          "required_model_assets": [
            {
              "id": "hidream-o1-image-dev-fp8-scaled",
              "name": "HiDream O1 Image Dev FP8 Scaled",
              "download_source": {
                "source_type": "huggingface",
                "repository_id": "Comfy-Org/HiDream-O1-Image",
                "file_path": "checkpoints/hidream_o1_image_dev_fp8_scaled.safetensors",
                "revision": "e469681accde36057e32e4a3125e39929a1bcd68"
              },
              "install_comfyui_relative_path": "models/checkpoints/hidream_o1_image_dev_fp8_scaled.safetensors"
            },
            {
              "id": "gemma4-e4b-it-fp8-scaled",
              "name": "Gemma 4 E4B IT FP8 Scaled",
              "download_source": {
                "source_type": "huggingface",
                "repository_id": "Comfy-Org/gemma-4",
                "file_path": "text_encoders/gemma4_e4b_it_fp8_scaled.safetensors",
                "revision": "c8b198a1279c02c9cf8aaa08171db4e2b0d15af9"
              },
              "install_comfyui_relative_path": "models/text_encoders/gemma4_e4b_it_fp8_scaled.safetensors"
            }
          ]
        }
      ]
    }
  ]
}
```

- [ ] **Step 5: Run workflow catalog tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml workflow_catalog
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/workflow_catalog/validation.rs src-tauri/src/workflow_catalog/service.rs bundled/workflow-catalog.json
git commit -m "feat(tauri): validate workflow revisions"
```

---

### Task 3: Persist WorkflowReference in Workspace Catalog

**Files:**

- Modify: `src-tauri/src/domain/workspace.rs`
- Modify: `src-tauri/src/workspace_catalog/schema.rs`
- Modify: `src-tauri/src/workspace_catalog/sqlite.rs`
- Modify: `src-tauri/src/workspace_catalog/service.rs`

- [ ] **Step 1: Update workspace domain tests first**

In `src-tauri/src/domain/workspace.rs`, change test setup to use:

```rust
use crate::domain::workflow_preset::WorkflowReference;

fn workflow_reference() -> WorkflowReference {
    WorkflowReference {
        id: "preset".to_string(),
        version: "1.0.0".to_string(),
    }
}
```

Update `Workspace` construction:

```rust
Workspace {
    id: "workspace-1".to_string(),
    workflow: workflow_reference(),
    state: WorkspaceState::NotProvisioned,
    runtime: WorkspaceRuntime::ProvisionedRemote(ProvisionedRemoteRuntime {
        placement: placement(),
        resources: ProvisionedRemoteResources {
            volume_id: None,
            provisioner_id: None,
            endpoint_id: Some("endpoint".to_string()),
        },
    }),
}
```

Add assertions:

```rust
assert_eq!(json["workflow"]["id"], "preset");
assert_eq!(json["workflow"]["version"], "1.0.0");
assert!(json.get("workflow_preset").is_none());
```

- [ ] **Step 2: Run the failing workspace domain test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml domain::workspace::tests::workspace_serializes_stable_state_separately_from_runtime
```

Expected: FAIL while `Workspace` still has `workflow_preset`.

- [ ] **Step 3: Change `Workspace`**

In `src-tauri/src/domain/workspace.rs`, replace:

```rust
use super::{provisioned_remote::ProvisionedRemoteRuntime, workflow_preset::WorkflowPreset};
```

with:

```rust
use super::{provisioned_remote::ProvisionedRemoteRuntime, workflow_preset::WorkflowReference};
```

Replace:

```rust
pub workflow_preset: WorkflowPreset,
```

with:

```rust
pub workflow: WorkflowReference,
```

- [ ] **Step 4: Update SQLite schema**

In `src-tauri/src/workspace_catalog/schema.rs`, keep:

```rust
const SCHEMA_VERSION: &str = "1";
```

Replace table column:

```sql
workflow_preset_json TEXT NOT NULL,
```

with:

```sql
workflow_id TEXT NOT NULL,
workflow_version TEXT NOT NULL,
```

Replace expected column metadata for `workflow_preset_json` with expected columns for `workflow_id` and `workflow_version`.

- [ ] **Step 5: Update SQLite repository read/write code**

In `src-tauri/src/workspace_catalog/sqlite.rs`, import `WorkflowReference` instead of `WorkflowPreset`.

Change SELECTs to:

```sql
SELECT id, runtime_type, provider_id, state, state_reason, workflow_id, workflow_version, runtime_json
```

Change insert columns to include:

```sql
workflow_id,
workflow_version,
```

Bind:

```rust
.bind(&workspace.workflow.id)
.bind(&workspace.workflow.version)
```

Change update SQL to set:

```sql
workflow_id = ?5,
workflow_version = ?6,
runtime_json = ?7,
updated_at = ?8
WHERE id = ?9
```

Read row fields:

```rust
let workflow_id = row
    .try_get::<String, _>("workflow_id")
    .map_err(|_| WorkspaceCatalogError::SchemaMismatch)?;
let workflow_version = row
    .try_get::<String, _>("workflow_version")
    .map_err(|_| WorkspaceCatalogError::SchemaMismatch)?;
```

Construct:

```rust
Ok(Workspace {
    id,
    workflow: WorkflowReference {
        id: workflow_id,
        version: workflow_version,
    },
    state,
    runtime,
})
```

- [ ] **Step 6: Update workspace catalog tests**

In `src-tauri/src/workspace_catalog/sqlite.rs` tests:

- Rename `workflow_preset_json` row helper fields to `workflow_id` and `workflow_version`.
- Replace corrupt workflow-preset-json tests with empty `workflow_id` and empty `workflow_version` corruption tests.
- Update schema mismatch fixtures to use `workflow_id TEXT NOT NULL, workflow_version TEXT NOT NULL`.
- Update update test to mutate `workspace.workflow.version = "1.0.1".to_string()`.

- [ ] **Step 7: Run workspace catalog tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml workspace_catalog
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/domain/workspace.rs src-tauri/src/workspace_catalog/schema.rs src-tauri/src/workspace_catalog/sqlite.rs src-tauri/src/workspace_catalog/service.rs
git commit -m "feat(tauri): persist workspace workflow references"
```

---

### Task 4: Resolve Workflows at Command and Response Boundaries

**Files:**

- Modify: `src-tauri/src/commands/types/catalog.rs`
- Modify: `src-tauri/src/commands/types/workspace.rs`
- Modify: `src-tauri/src/commands/catalog.rs`
- Modify: `src-tauri/src/commands/workspaces.rs`

- [ ] **Step 1: Add DTO shapes for revisions and resolved workflow responses**

In `src-tauri/src/commands/types/catalog.rs`, replace flat `WorkflowPresetResponse` fields with:

```rust
pub struct WorkflowPresetResponse {
    pub id: String,
    pub name: String,
    pub execution_type: WorkflowExecutionTypeDto,
    pub revisions: Vec<WorkflowRevisionResponse>,
}

pub struct WorkflowRevisionResponse {
    pub version: String,
    pub requires_hugging_face_api_key: bool,
    pub remote_runtime_requirements: RemoteRuntimeRequirementsResponse,
    pub required_model_assets: Vec<ModelAssetResponse>,
}
```

Add flattened resolved response for workspace DTOs:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowPresetResolvedResponse {
    pub id: String,
    pub version: String,
    pub name: String,
    pub execution_type: WorkflowExecutionTypeDto,
    pub requires_hugging_face_api_key: bool,
    pub remote_runtime_requirements: RemoteRuntimeRequirementsResponse,
    pub required_model_assets: Vec<ModelAssetResponse>,
}
```

Implement `From<WorkflowRevision>` and `From<WorkflowPresetResolved>`.

- [ ] **Step 2: Update create request DTO**

In `src-tauri/src/commands/types/workspace.rs`, change:

```rust
pub struct CreateWorkspaceRequest {
    pub workflow_preset_id: String,
    pub remote_placement: RemotePlacementPlanInput,
}
```

- [ ] **Step 3: Update workspace response conversion**

Change `WorkspaceResponse.workflow_preset` type to `WorkflowPresetResolvedResponse`.

Remove `impl From<Workspace> for WorkspaceResponse`. Add:

```rust
impl WorkspaceResponse {
    pub fn from_parts(
        workspace: Workspace,
        workflow: crate::domain::workflow_preset::WorkflowPresetResolved,
    ) -> Self {
        Self {
            id: workspace.id,
            workflow_preset: workflow.into(),
            state: workspace.state.into(),
            runtime: workspace.runtime.into(),
        }
    }
}
```

Change `WorkspaceCatalogResponse` construction to accept already resolved responses instead of implementing `From<WorkspaceCatalog>`.

- [ ] **Step 4: Update command handlers**

In `src-tauri/src/commands/workspaces.rs`, pass the preset id into provisioned-remote creation:

```rust
workflow_preset_id: request.workflow_preset_id,
```

Provisioned-remote creation resolves the latest workflow revision and persists that exact reference. Return:

```rust
Ok(WorkspaceResponse::from_parts(workspace, workflow))
```

In `src-tauri/src/commands/catalog.rs`, update `get_workspace_catalog`:

```rust
let workflow_catalog = state.workflow_catalog.get_workflow_catalog()?;
let catalog = state.workspace_catalog.list_workspaces().await?;
let workspaces = catalog
    .workspaces
    .into_iter()
    .map(|workspace| {
        let workflow = workflow_catalog.resolve(&workspace.workflow).ok_or_else(|| {
            NativeCommandError::new(
                NativeCommandErrorCode::WorkflowCatalogInvalid,
                "workspace workflow reference was not found",
            )
        })?;
        Ok(WorkspaceResponse::from_parts(workspace, workflow))
    })
    .collect::<CommandResult<Vec<_>>>()?;

Ok(WorkspaceCatalogResponse { workspaces })
```

- [ ] **Step 5: Run command type tests through compilation**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml commands
```

Expected: PASS or compile failures only in provisioned-remote code that still expects `workflow_preset`; those are handled in Task 5.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/types/catalog.rs src-tauri/src/commands/types/workspace.rs src-tauri/src/commands/catalog.rs src-tauri/src/commands/workspaces.rs
git commit -m "feat(tauri): resolve workflow references for commands"
```

---

### Task 5: Use Resolved Workflows in Provisioned Remote Lifecycle

**Files:**

- Modify: `src-tauri/src/provisioned_remote/service.rs`
- Modify: `src-tauri/src/provisioned_remote/contracts.rs`
- Modify: `src-tauri/src/provisioned_remote/lifecycle/provision.rs`
- Modify: `src-tauri/src/provisioned_remote/test_support.rs`
- Modify: `src-tauri/src/app/events.rs`

- [ ] **Step 1: Update create request and service validation**

In `src-tauri/src/provisioned_remote/service.rs`, change imports to use `WorkflowPresetResolved` and `WorkflowReference`.

Change request:

```rust
pub struct CreateProvisionedRemoteWorkspaceRequest {
    pub workspace_id: String,
    pub workflow: WorkflowReference,
    pub resolved_workflow: WorkflowPresetResolved,
    pub remote_placement: RemotePlacementPlan,
}
```

Validate placement against `request.resolved_workflow.remote_runtime_requirements`.

Create workspace with:

```rust
let workspace = Workspace {
    id: request.workspace_id,
    workflow: request.workflow,
    state: WorkspaceState::NotProvisioned,
    runtime: WorkspaceRuntime::ProvisionedRemote(ProvisionedRemoteRuntime {
        placement: request.remote_placement,
        resources: ProvisionedRemoteResources {
            volume_id: None,
            provisioner_id: None,
            endpoint_id: None,
        },
    }),
};
```

- [ ] **Step 2: Change contract resolver**

In `src-tauri/src/provisioned_remote/contracts.rs`, change:

```rust
pub fn resolve(
    workflow: &WorkflowPresetResolved,
    runtime: &ProvisionedRemoteRuntime,
) -> Result<ProvisionedRemoteRuntimeContracts, ProvisionedRemoteError>
```

Resolve provider requirements from:

```rust
workflow
    .remote_runtime_requirements
    .resolve_provider_requirements(runtime.provider_id())
```

- [ ] **Step 3: Resolve workflow during provision lifecycle**

In `src-tauri/src/provisioned_remote/lifecycle/provision.rs`, before contract resolution, read the workflow catalog:

```rust
let workflow_catalog = BundledWorkflowCatalogReader
    .read_workflow_catalog()
    .map_err(|_| ProvisionedRemoteError::InvalidRuntimeState)?;
let resolved_workflow = workflow_catalog
    .resolve(&workspace.workflow)
    .ok_or(ProvisionedRemoteError::InvalidRuntimeState)?;
```

Use:

```rust
let contracts = ProvisionedRemoteContractResolver::resolve(&resolved_workflow, &runtime_state)?;
```

Pass model assets to the provisioner from:

```rust
required_model_assets: resolved_workflow.required_model_assets.clone(),
```

- [ ] **Step 4: Adjust event conversion**

`TauriProvisionedRemoteEventSink` currently converts `Workspace` directly to `WorkspaceResponse`. Because that now needs a resolved workflow, change event payload construction to resolve via `WorkflowCatalogService::new().get_workflow_catalog()`:

```rust
let workspace = *workspace;
let event = WorkflowCatalogService::new()
    .get_workflow_catalog()
    .ok()
    .and_then(|catalog| catalog.resolve(&workspace.workflow))
    .map(|workflow| WorkspaceChangedEvent {
        workspace_id,
        workspace: WorkspaceResponse::from_parts(workspace, workflow),
    });
if let Some(event) = event {
    let _ = event.emit(&self.app_handle);
}
```

Do not emit a malformed workspace event if catalog resolution fails.

- [ ] **Step 5: Update provisioned remote tests**

In `src-tauri/src/provisioned_remote/test_support.rs` and `service.rs` tests, replace helper `workflow_preset()` with:

```rust
fn workflow_reference() -> WorkflowReference {
    WorkflowReference {
        id: "preset".to_string(),
        version: "1.0.0".to_string(),
    }
}

fn resolved_workflow() -> WorkflowPresetResolved {
    WorkflowPresetResolved {
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
                    id: "endpoint".to_string(),
                    version: "1.0.0".to_string(),
                },
                provisioner_contract: RuntimeContractReference {
                    id: "provisioner".to_string(),
                    version: "1.0.0".to_string(),
                },
            }],
        },
        required_model_assets: Vec::new(),
    }
}
```

Update assertions from `workspace.workflow_preset.id` to `workspace.workflow.id`.

- [ ] **Step 6: Run provisioned remote tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml provisioned_remote
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/provisioned_remote/service.rs src-tauri/src/provisioned_remote/contracts.rs src-tauri/src/provisioned_remote/lifecycle/provision.rs src-tauri/src/provisioned_remote/test_support.rs src-tauri/src/app/events.rs
git commit -m "feat(tauri): provision with resolved workflow references"
```

---

### Task 6: Regenerate Bindings and Update Frontend Diagnostics Request

**Files:**

- Modify: `src/generated/commands.ts`
- Modify: `src/pages/home/ui/home-page.tsx`

- [ ] **Step 1: Regenerate command bindings**

Run:

```bash
bun run codegen:commands
```

Expected: `src/generated/commands.ts` includes only `workflowPresetId` on `CreateWorkspaceRequest`, workflow catalog presets with `revisions`, and workspace responses with `workflow: WorkflowReferenceResponse`.

- [ ] **Step 2: Keep home diagnostics create request default on preset id only**

In `src/pages/home/ui/home-page.tsx`, keep the sample create request as:

```ts
const request = {
  workflowPresetId: "",
  remotePlacement: {
    gpuCloudProviderId: "runpod",
    datacenterId: "",
    gpuId: "",
    volumeSizeBytes: 0,
    keepAliveLimits: null,
  },
};
```

- [ ] **Step 3: Run frontend checks affected by generated contracts**

Run:

```bash
bun run build
```

Expected: PASS. If unrelated lint drift exists, record it separately and do not broaden this task.

- [ ] **Step 4: Commit**

```bash
git add src/generated/commands.ts src/pages/home/ui/home-page.tsx
git commit -m "chore(frontend): update workflow revision command bindings"
```

---

### Task 7: Final Native Verification

**Files:**

- No planned source edits.

- [ ] **Step 1: Run full native tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 2: Run Rust formatting check**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: PASS.

- [ ] **Step 3: Run clippy**

Run:

```bash
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: PASS.

- [ ] **Step 4: Run frontend generated-contract checks**

Run:

```bash
bun run build
```

Expected: PASS.

- [ ] **Step 5: Commit any final mechanical fixes**

If formatting or generated files changed:

```bash
git add src-tauri src/generated/commands.ts src/pages/home/ui/home-page.tsx bundled/workflow-catalog.json
git commit -m "chore(tauri): finish workflow reference versioning"
```

If no files changed, do not create an empty commit.
