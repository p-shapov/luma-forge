# RunPod Runtime Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the generic `GpuCloudProviderId` / `provisioned_remote` boundary with explicit RunPod runtime domain, persistence, lifecycle, command, and frontend contracts.

**Architecture:** Keep `WorkspaceRuntime` as the product-level runtime enum, but make its current variant `Runpod(RunpodRuntime)`. Move catalog requirements, placement, resources, lifecycle steps, service/client traits, SQLite encoding, and command DTOs to RunPod-specific names and RunPod units. Keep a narrow `RunpodRuntimeClient` trait only as an I/O seam for tests, not as a cloud-provider abstraction.

**Tech Stack:** Rust, Tauri commands, serde, Specta, sqlx SQLite, existing bundled JSON catalogs, TypeScript generated command bindings, React diagnostics page.

---

## File Structure

- Modify `src-tauri/src/domain/workflow_preset.rs`: replace `RemoteRuntimeRequirements` and provider requirements with `RunpodRuntimeRequirements`; move `required_volume_size_gb` onto `WorkflowRevision`.
- Modify `src-tauri/src/workflow_catalog/validation.rs`: validate RunPod requirements and GB volume sizes.
- Modify `bundled/workflow-catalog.json`: replace `remote_runtime_requirements` with `required_volume_size_gb` and `runpod_runtime_requirements`.
- Rename conceptually, and as practical during implementation, `src-tauri/src/domain/provisioned_remote/*` toward RunPod runtime types. If physical file renames are too noisy in one commit, first update exported type names, then rename folders in a later cleanup commit.
- Modify `src-tauri/src/domain/workspace.rs`: change `WorkspaceRuntime::ProvisionedRemote` to `WorkspaceRuntime::Runpod`.
- Modify `src-tauri/src/workspace_catalog/schema.rs`: remove `provider_id`; keep `runtime_type`, `runtime_json`, workflow columns, state columns.
- Modify `src-tauri/src/workspace_catalog/runtime.rs` and `src-tauri/src/workspace_catalog/runtimes/provisioned_remote.rs`: encode/decode `runtime_type = "runpod"` and `RunpodRuntime`.
- Modify `src-tauri/src/workspace_catalog/sqlite.rs`: remove `provider_id` reads/writes/validation and update tests.
- Modify `src-tauri/src/provisioned_remote/provider.rs`: replace provider-shaped traits with `RunpodRuntimeClient` and RunPod-specific parameter/result structs.
- Modify `src-tauri/src/provisioned_remote/providers/runpod/*`: return/use RunPod GB fields, RunPod placement option names, and endpoint creation result containing endpoint id plus template id.
- Modify `src-tauri/src/provisioned_remote/registry.rs`: delete once `RunpodRuntimeService` receives a concrete `Arc<dyn RunpodRuntimeClient>`.
- Modify `src-tauri/src/provisioned_remote/service.rs`: rename to `RunpodRuntimeService` in code, requests, tests, and app state.
- Modify `src-tauri/src/provisioned_remote/lifecycle/{provision,cleanup,delete,helpers,runner}.rs`: use RunPod runtime/resources/client and RunPod lifecycle names.
- Modify `src-tauri/src/provisioned_remote/contracts.rs`: resolve `runpod_runtime_requirements`.
- Modify `src-tauri/src/commands/{catalog,workspaces}.rs`: rename commands to `get_runpod_placement_options` and `create_runpod_workspace`.
- Modify `src-tauri/src/commands/types/{catalog,placement,provider,workspace}.rs`: remove provider DTOs; add RunPod-specific DTOs and `placement` request field.
- Modify `src-tauri/src/app/{bootstrap,state}.rs`: wire `RunpodRuntimeService` with the RunPod client directly.
- Modify `src-tauri/src/lib.rs`: export renamed Tauri commands.
- Regenerate `src/generated/commands.ts` with `bun run codegen:commands`.
- Modify `src/pages/home/ui/home-page.tsx`: use generated RunPod command names and `placement` input.

---

### Task 1: Workflow Catalog RunPod Requirements

**Files:**
- Modify: `src-tauri/src/domain/workflow_preset.rs`
- Modify: `src-tauri/src/workflow_catalog/validation.rs`
- Modify: `bundled/workflow-catalog.json`

- [ ] **Step 1: Write failing workflow domain tests**

In `src-tauri/src/domain/workflow_preset.rs`, update the test helper revision to the target shape:

```rust
fn revision(version: &str, volume_size_gb: u64) -> WorkflowRevision {
    WorkflowRevision {
        version: version.to_string(),
        requires_hugging_face_api_key: true,
        required_volume_size_gb: volume_size_gb,
        runpod_runtime_requirements: RunpodRuntimeRequirements {
            endpoint_contract: RuntimeContractReference {
                id: "endpoint".to_string(),
                version: "1.0.0".to_string(),
            },
            provisioner_contract: RuntimeContractReference {
                id: "provisioner".to_string(),
                version: "1.0.0".to_string(),
            },
        },
        required_model_assets: Vec::new(),
    }
}
```

Update existing assertions from:

```rust
resolved
    .remote_runtime_requirements
    .required_base_volume_size_bytes
```

to:

```rust
resolved.required_volume_size_gb
```

- [ ] **Step 2: Run focused failing domain tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml domain::workflow_preset::tests
```

Expected: FAIL because `RunpodRuntimeRequirements` and `required_volume_size_gb` are not implemented yet.

- [ ] **Step 3: Implement workflow requirement types**

In `src-tauri/src/domain/workflow_preset.rs`, remove `RemoteProviderRuntimeRequirements`, `RemoteRuntimeRequirements`, and the `GpuCloudProviderId` import. Add:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunpodRuntimeRequirements {
    pub endpoint_contract: RuntimeContractReference,
    pub provisioner_contract: RuntimeContractReference,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRevision {
    pub version: String,
    pub requires_hugging_face_api_key: bool,
    pub required_volume_size_gb: u64,
    pub runpod_runtime_requirements: RunpodRuntimeRequirements,
    pub required_model_assets: Vec<ModelAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowPresetResolved {
    pub id: String,
    pub version: String,
    pub name: String,
    pub execution_type: WorkflowExecutionType,
    pub requires_hugging_face_api_key: bool,
    pub required_volume_size_gb: u64,
    pub runpod_runtime_requirements: RunpodRuntimeRequirements,
    pub required_model_assets: Vec<ModelAsset>,
}
```

Update `WorkflowCatalog::resolve` and `resolve_latest` to copy `required_volume_size_gb` and `runpod_runtime_requirements` from the selected revision.

- [ ] **Step 4: Update workflow validation tests**

In `src-tauri/src/workflow_catalog/validation.rs`, update imports:

```rust
use crate::domain::{
    runtime_contract::{RuntimeContract, RuntimeContractReference, RuntimeContractRevision},
    workflow_preset::{
        ModelAsset, ModelAssetSource, RunpodRuntimeRequirements, WorkflowExecutionType,
        WorkflowRevision,
    },
};
```

Update `valid_revision`:

```rust
fn valid_revision(version: &str) -> WorkflowRevision {
    WorkflowRevision {
        version: version.to_string(),
        requires_hugging_face_api_key: true,
        required_volume_size_gb: 19,
        runpod_runtime_requirements: RunpodRuntimeRequirements {
            endpoint_contract: RuntimeContractReference {
                id: "comfyui-py312-cu126-torch291".to_string(),
                version: "1.0.15".to_string(),
            },
            provisioner_contract: RuntimeContractReference {
                id: "luma-forge-provisioner".to_string(),
                version: "1.0.6".to_string(),
            },
        },
        required_model_assets: vec![valid_asset()],
    }
}
```

Add a validation test:

```rust
#[test]
fn validate_workflows_rejects_zero_required_volume_size_gb() {
    let mut workflow = valid_workflow("workflow");
    workflow.revisions[0].required_volume_size_gb = 0;

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

- [ ] **Step 5: Implement workflow validation**

In `validate_workflows`, replace provider-loop validation with direct RunPod references:

```rust
if revision.required_volume_size_gb == 0
    || endpoint_contract_catalog
        .resolve(&revision.runpod_runtime_requirements.endpoint_contract)
        .is_none()
    || provisioner_contract_catalog
        .resolve(&revision.runpod_runtime_requirements.provisioner_contract)
        .is_none()
{
    return Err(WorkflowCatalogError::ValidationFailed);
}
```

- [ ] **Step 6: Update bundled workflow catalog**

In `bundled/workflow-catalog.json`, change the revision block to:

```json
"required_volume_size_gb": 19,
"runpod_runtime_requirements": {
  "endpoint_contract": {
    "id": "comfyui-py312-cu126-torch291",
    "version": "1.0.16"
  },
  "provisioner_contract": {
    "id": "luma-forge-provisioner",
    "version": "1.0.8"
  }
}
```

Remove the old `remote_runtime_requirements` object.

- [ ] **Step 7: Run workflow tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml domain::workflow_preset::tests workflow_catalog::validation::tests
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/domain/workflow_preset.rs src-tauri/src/workflow_catalog/validation.rs bundled/workflow-catalog.json
git commit -m "refactor(runpod): model workflow runtime requirements"
```

---

### Task 2: RunPod Runtime Domain And Workspace Persistence

**Files:**
- Modify: `src-tauri/src/domain/provisioned_remote/{mod,placement,provider,runtime}.rs`
- Modify: `src-tauri/src/domain/workspace.rs`
- Modify: `src-tauri/src/workspace_catalog/{runtime,schema,sqlite}.rs`
- Modify: `src-tauri/src/workspace_catalog/runtimes/provisioned_remote.rs`
- Modify tests in the same files.

- [ ] **Step 1: Write failing runtime serialization tests**

In `src-tauri/src/domain/workspace.rs`, replace the runtime serialization test setup with:

```rust
runtime: WorkspaceRuntime::Runpod(RunpodRuntime {
    placement: runpod_placement(),
    resources: RunpodResources {
        network_volume_id: None,
        provisioner_pod_id: None,
        endpoint_id: Some("endpoint".to_string()),
        template_id: Some("template".to_string()),
    },
}),
```

Assert:

```rust
assert_eq!(json["runtime"]["runtime_type"], "runpod");
assert_eq!(json["runtime"]["resources"]["endpoint_id"], "endpoint");
assert_eq!(json["runtime"]["resources"]["template_id"], "template");
assert!(json["runtime"]["placement"].get("gpu_cloud_provider_id").is_none());
assert_eq!(json["runtime"]["placement"]["volume_size_gb"], 19);
```

- [ ] **Step 2: Run focused failing runtime tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml domain::workspace::tests
```

Expected: FAIL because `WorkspaceRuntime::Runpod`, `RunpodRuntime`, and `RunpodResources` do not exist.

- [ ] **Step 3: Replace runtime domain types**

In `src-tauri/src/domain/provisioned_remote/placement.rs`, replace remote placement structs with:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunpodGpuPlacementOption {
    pub id: String,
    pub name: String,
    pub vram_gb: u64,
    pub availability_score: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunpodDatacenterPlacementOption {
    pub id: String,
    pub name: String,
    pub gpu_options: Vec<RunpodGpuPlacementOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunpodPlacementOptions {
    pub max_network_volume_size_gb: Option<u64>,
    pub datacenters: Vec<RunpodDatacenterPlacementOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunpodEndpointKeepAliveLimits {
    pub default_seconds: u32,
    pub min_seconds: u32,
    pub max_seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunpodPlacementPlan {
    pub data_center_id: String,
    pub gpu_type_id: String,
    pub volume_size_gb: u64,
    pub keep_alive_limits: Option<RunpodEndpointKeepAliveLimits>,
}
```

In `src-tauri/src/domain/provisioned_remote/runtime.rs`, replace runtime structs with:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunpodRuntime {
    pub placement: RunpodPlacementPlan,
    pub resources: RunpodResources,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunpodResources {
    pub network_volume_id: Option<String>,
    pub provisioner_pod_id: Option<String>,
    pub endpoint_id: Option<String>,
    pub template_id: Option<String>,
}

impl RunpodResources {
    pub fn is_empty(&self) -> bool {
        self.network_volume_id.is_none()
            && self.provisioner_pod_id.is_none()
            && self.endpoint_id.is_none()
            && self.template_id.is_none()
    }
}
```

In `src-tauri/src/domain/provisioned_remote/provider.rs`, delete `GpuCloudProviderId`. Keep `ProviderApiError` for now; Task 5 renames error variants.

- [ ] **Step 4: Update workspace runtime enum**

In `src-tauri/src/domain/workspace.rs`, change:

```rust
pub enum WorkspaceRuntime {
    Runpod(RunpodRuntime),
}
```

Update imports from `ProvisionedRemoteRuntime` to `RunpodRuntime`.

- [ ] **Step 5: Update SQLite schema tests first**

In `src-tauri/src/workspace_catalog/sqlite.rs`, update schema-oriented tests so row assertions do not read `provider_id` and expect:

```rust
assert_eq!(row.get::<String, _>("runtime_type"), "runpod");
```

Update test workspace fixtures to create `WorkspaceRuntime::Runpod`.

- [ ] **Step 6: Remove provider_id from persistence**

In `src-tauri/src/workspace_catalog/schema.rs`:

- remove `provider_id TEXT NOT NULL`
- remove `ExpectedColumn { name: "provider_id", ... }`
- remove `idx_workspaces_provider_id`

In `src-tauri/src/workspace_catalog/runtime.rs`, change `EncodedWorkspaceRuntime` to:

```rust
pub struct EncodedWorkspaceRuntime {
    pub runtime_type: String,
    pub runtime_json: String,
}
```

In `src-tauri/src/workspace_catalog/runtimes/provisioned_remote.rs`, set:

```rust
pub const RUNTIME_TYPE: &str = "runpod";
```

and encode/decode `RunpodRuntime`.

In `src-tauri/src/workspace_catalog/sqlite.rs`, remove `provider_id` from SELECT, INSERT, UPDATE, `workspace_from_row`, and provider consistency checks.

- [ ] **Step 7: Run persistence tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml domain::workspace::tests workspace_catalog::sqlite::tests
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/domain/provisioned_remote src-tauri/src/domain/workspace.rs src-tauri/src/workspace_catalog
git commit -m "refactor(runpod): persist runpod runtime state"
```

---

### Task 3: RunPod Runtime Client And API Mapping

**Files:**
- Modify: `src-tauri/src/provisioned_remote/provider.rs`
- Modify: `src-tauri/src/provisioned_remote/providers/runpod/{api,mapping,mod,config}.rs`
- Modify: `src-tauri/src/provisioned_remote/test_support.rs`

- [ ] **Step 1: Write failing RunPod mapping tests**

In `src-tauri/src/provisioned_remote/providers/runpod/mapping.rs`, replace `bytes_to_runpod_volume_gb` tests with direct GB serialization tests. Add:

```rust
#[test]
fn placement_response_preserves_runpod_gb_units() {
    let response = GraphqlResponse {
        data: Some(PlacementQueryData {
            gpu_types: vec![PlacementGpuType {
                id: "NVIDIA RTX A5000".to_string(),
                display_name: "RTX A5000".to_string(),
                memory_gb: 24,
            }],
            datacenters: vec![PlacementDatacenter {
                id: "EU-RO-1".to_string(),
                name: "EU RO 1".to_string(),
                gpu_availability: vec![PlacementGpuAvailability {
                    gpu_type_id: "NVIDIA RTX A5000".to_string(),
                    available: true,
                    stock_status: Some("High".to_string()),
                }],
            }],
        }),
        errors: Vec::new(),
    };

    let options = map_placement_response(response).expect("placement options");

    assert_eq!(options.max_network_volume_size_gb, None);
    assert_eq!(options.datacenters[0].gpu_options[0].vram_gb, 24);
}
```

- [ ] **Step 2: Run focused failing RunPod provider tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml provisioned_remote::providers::runpod
```

Expected: FAIL because mapping still returns byte fields and provider params still use byte fields.

- [ ] **Step 3: Replace provider traits with RunPod client trait**

In `src-tauri/src/provisioned_remote/provider.rs`, replace provider traits with:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateRunpodNetworkVolumeParams {
    pub workspace_id: String,
    pub data_center_id: String,
    pub size_gb: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartRunpodProvisionerPodParams {
    pub workspace_id: String,
    pub data_center_id: String,
    pub network_volume_id: String,
    pub provisioner_image_ref: String,
    pub requires_hugging_face_api_key: bool,
    pub required_model_assets: Vec<ModelAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateRunpodEndpointParams {
    pub workspace_id: String,
    pub data_center_id: String,
    pub gpu_type_id: String,
    pub network_volume_id: String,
    pub endpoint_image_ref: String,
    pub keep_alive_limits: Option<RunpodEndpointKeepAliveLimits>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateRunpodEndpointResult {
    pub endpoint_id: String,
    pub template_id: String,
}

pub trait RunpodRuntimeClient: Send + Sync {
    fn placement_options<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<RunpodPlacementOptions, ProvisionedRemoteError>>;

    fn create_network_volume<'a>(
        &'a self,
        params: CreateRunpodNetworkVolumeParams,
    ) -> AppFuture<'a, Result<String, ProvisionedRemoteError>>;

    fn delete_network_volume<'a>(
        &'a self,
        network_volume_id: &'a str,
    ) -> AppFuture<'a, Result<(), ProvisionedRemoteError>>;

    fn start_provisioner_pod<'a>(
        &'a self,
        params: StartRunpodProvisionerPodParams,
    ) -> AppFuture<'a, Result<String, ProvisionedRemoteError>>;

    fn terminate_provisioner_pod<'a>(
        &'a self,
        provisioner_pod_id: &'a str,
    ) -> AppFuture<'a, Result<(), ProvisionedRemoteError>>;

    fn get_provisioner_status<'a>(
        &'a self,
        workspace_id: &'a str,
        provisioner_pod_id: &'a str,
    ) -> AppFuture<'a, Result<ProvisionedRemoteProvisionerStatus, ProvisionedRemoteError>>;

    fn create_serverless_endpoint<'a>(
        &'a self,
        params: CreateRunpodEndpointParams,
    ) -> AppFuture<'a, Result<CreateRunpodEndpointResult, ProvisionedRemoteError>>;

    fn delete_serverless_endpoint<'a>(
        &'a self,
        endpoint_id: &'a str,
    ) -> AppFuture<'a, Result<(), ProvisionedRemoteError>>;

    fn delete_template<'a>(
        &'a self,
        template_id: &'a str,
    ) -> AppFuture<'a, Result<(), ProvisionedRemoteError>>;
}
```

- [ ] **Step 4: Update RunPod API mapping**

In `mapping.rs`:

- delete `bytes_to_runpod_volume_gb`
- map `memoryInGb` to `vram_gb`
- map max volume config to `max_network_volume_size_gb`
- keep `NetworkVolumeCreateBody.size` in GB

In `config.rs`, replace:

```rust
pub const NETWORK_VOLUME_MAX_SIZE_BYTES: u64 = 4_000 * 1_000_000_000;
```

with:

```rust
pub const NETWORK_VOLUME_MAX_SIZE_GB: u64 = 4_000;
```

- [ ] **Step 5: Update endpoint creation result**

In `api.rs`, change `RunpodEndpoint`:

```rust
pub struct RunpodEndpoint {
    pub id: String,
    pub template_id: String,
    pub url: String,
}
```

In `HttpRunpodApi::create_endpoint`, return:

```rust
Ok(RunpodEndpoint {
    id: endpoint_response.id,
    template_id: template_response.id,
    url: endpoint_response.url.unwrap_or_default(),
})
```

Add a dedicated API method for deleting templates if one does not exist:

```rust
fn delete_template<'a>(
    &'a self,
    template_id: &'a str,
) -> AppFuture<'a, Result<(), ProvisionedRemoteError>>;
```

Implement it with:

```rust
self.delete_rest(
    &format!("/templates/{template_id}"),
    RunpodOperation::DeleteTemplate,
)
.await
```

- [ ] **Step 6: Implement `RunpodRuntimeClient` for the RunPod provider**

In `providers/runpod/mod.rs`, implement `RunpodRuntimeClient` for `RunpodProvisionedRemoteProvider`. Rename methods to the trait names and update request fields:

```rust
CreateNetworkVolumeRequest {
    datacenter_id: params.data_center_id,
    name: mapping::workspace_resource_name(&params.workspace_id, "volume"),
    size_gb: params.size_gb,
}
```

For endpoint creation, return:

```rust
Ok(CreateRunpodEndpointResult {
    endpoint_id: endpoint.id,
    template_id: endpoint.template_id,
})
```

- [ ] **Step 7: Update fake client/test support**

In `src-tauri/src/provisioned_remote/test_support.rs`, replace `FakeProvider` trait implementation with `RunpodRuntimeClient`. Rename fake state fields:

- `create_volume_requests` -> `create_network_volume_requests`
- `provisioner_id` -> `provisioner_pod_id`
- `volume_id` -> `network_volume_id`

Ensure fake endpoint creation returns:

```rust
CreateRunpodEndpointResult {
    endpoint_id: "endpoint".to_string(),
    template_id: "template".to_string(),
}
```

- [ ] **Step 8: Run RunPod provider tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml provisioned_remote::providers::runpod provisioned_remote::test_support
```

Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/provisioned_remote/provider.rs src-tauri/src/provisioned_remote/providers/runpod src-tauri/src/provisioned_remote/test_support.rs
git commit -m "refactor(runpod): introduce runtime client"
```

---

### Task 4: RunPod Runtime Service And Contract Resolution

**Files:**
- Modify: `src-tauri/src/provisioned_remote/service.rs`
- Modify: `src-tauri/src/provisioned_remote/contracts.rs`
- Delete or stop using: `src-tauri/src/provisioned_remote/registry.rs`
- Modify: `src-tauri/src/app/{bootstrap,state}.rs`
- Modify tests in `service.rs` and `test_support.rs`.

- [ ] **Step 1: Write failing service tests for volume requirement**

In `src-tauri/src/provisioned_remote/service.rs`, add:

```rust
#[test]
fn create_runpod_workspace_rejects_volume_below_workflow_requirement() {
    let fixture = ServiceFixture::default();
    let service = fixture.service();
    let mut request = draft_create_request("workspace-1");
    request.placement.volume_size_gb = 1;

    let error = block_on(service.create_runpod_workspace(request))
        .expect_err("volume below workflow requirement should fail");

    assert_eq!(error, ProvisionedRemoteError::InvalidRuntimeState);
    assert_eq!(fixture.workspace_repository.snapshot().workspaces, Vec::new());
}
```

Use the existing fixture pattern and adapt names to the actual fixture helper names in the file.

- [ ] **Step 2: Run focused failing service test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml provisioned_remote::service::tests::create_runpod_workspace_rejects_volume_below_workflow_requirement
```

Expected: FAIL because the service/request names and validation do not exist yet.

- [ ] **Step 3: Rename service request and service methods**

In `service.rs`, replace:

```rust
pub struct CreateProvisionedRemoteWorkspaceRequest {
    pub workspace_id: String,
    pub workflow_preset_id: String,
    pub remote_placement: RemotePlacementPlan,
}
```

with:

```rust
pub struct CreateRunpodWorkspaceRequest {
    pub workspace_id: String,
    pub workflow_preset_id: String,
    pub placement: RunpodPlacementPlan,
}
```

Rename:

- `ProvisionedRemoteService` -> `RunpodRuntimeService`
- `create_workspace` -> `create_runpod_workspace`
- `get_provider_placement_options` -> `get_runpod_placement_options`

Replace the provider registry field with:

```rust
runpod_client: Arc<dyn RunpodRuntimeClient>,
```

- [ ] **Step 4: Implement create validation and runtime construction**

Inside `create_runpod_workspace`, after resolving latest workflow:

```rust
if request.placement.volume_size_gb < workflow.required_volume_size_gb {
    return Err(ProvisionedRemoteError::InvalidRuntimeState);
}
```

Persist:

```rust
runtime: WorkspaceRuntime::Runpod(RunpodRuntime {
    placement: request.placement,
    resources: RunpodResources {
        network_volume_id: None,
        provisioner_pod_id: None,
        endpoint_id: None,
        template_id: None,
    },
}),
```

- [ ] **Step 5: Update contract resolver**

In `contracts.rs`, replace provider-specific resolution with:

```rust
let endpoint_contract = endpoint_catalog
    .resolve(&workflow.runpod_runtime_requirements.endpoint_contract)
    .ok_or(ProvisionedRemoteError::InvalidRuntimeState)?;
let provisioner_contract = provisioner_catalog
    .resolve(&workflow.runpod_runtime_requirements.provisioner_contract)
    .ok_or(ProvisionedRemoteError::InvalidRuntimeState)?;
```

Remove the runtime argument from the resolver if it is only used for provider lookup.

- [ ] **Step 6: Update app bootstrap**

In `src-tauri/src/app/bootstrap.rs`, remove `ProvisionedRemoteProviderRegistry` construction and pass:

```rust
Arc::new(runpod_provider)
```

directly to `RunpodRuntimeService::new(...)`.

In `src-tauri/src/app/state.rs`, rename the type alias and field:

```rust
pub type RunpodRuntimeAppService =
    RunpodRuntimeService<SqliteWorkspaceCatalogRepository, SqliteLifecycleJournalRepository>;

pub runpod_runtime: RunpodRuntimeAppService,
```

- [ ] **Step 7: Run service tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml provisioned_remote::service::tests provisioned_remote::contracts
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/provisioned_remote/service.rs src-tauri/src/provisioned_remote/contracts.rs src-tauri/src/provisioned_remote/registry.rs src-tauri/src/app
git commit -m "refactor(runpod): make runtime service concrete"
```

---

### Task 5: RunPod Lifecycle Provision/Cleanup/Delete

**Files:**
- Modify: `src-tauri/src/domain/provisioned_remote/lifecycle.rs`
- Modify: `src-tauri/src/domain/lifecycle_operation.rs`
- Modify: `src-tauri/src/lifecycle_journal/payloads/provisioned_remote.rs`
- Modify: `src-tauri/src/provisioned_remote/lifecycle/{provision,cleanup,delete,helpers,runner}.rs`
- Modify: `src-tauri/src/provisioned_remote/errors.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify lifecycle tests in `service.rs`, `sqlite.rs`, and `commands/types/workspace.rs`.

- [ ] **Step 1: Write failing lifecycle tests for template persistence**

In `src-tauri/src/provisioned_remote/service.rs`, update the provisioning success test to assert:

```rust
let WorkspaceRuntime::Runpod(runtime) = workspace.runtime;
assert_eq!(runtime.resources.network_volume_id, Some("volume".to_string()));
assert_eq!(runtime.resources.provisioner_pod_id, None);
assert_eq!(runtime.resources.endpoint_id, Some("endpoint".to_string()));
assert_eq!(runtime.resources.template_id, Some("template".to_string()));
```

Add cleanup/delete assertions that endpoint and template deletion are separate calls in fake client state.

- [ ] **Step 2: Run focused failing lifecycle tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml provisioned_remote::service::tests::provision_workspace_completes_and_persists_ready_workspace
```

Expected: FAIL because lifecycle still persists old resource fields and endpoint creation returns only endpoint id.

- [ ] **Step 3: Rename lifecycle domain names**

In `lifecycle.rs`, rename step enum variants:

```rust
pub enum ProvisionedRemoteProvisionStep {
    CreateNetworkVolume,
    StartProvisionerPod,
    PollProvisioner,
    TerminateProvisionerPod,
    CreateEndpoint,
}

pub enum ProvisionedRemoteCleanupStep {
    DeleteEndpoint,
    DeleteTemplate,
    TerminateProvisionerPod,
    DeleteNetworkVolume,
}

pub enum ProvisionedRemoteDeleteStep {
    DeleteEndpoint,
    DeleteTemplate,
    TerminateProvisionerPod,
    DeleteNetworkVolume,
    DeleteLocalWorkspace,
}
```

If this task also renames the enum type prefixes from `ProvisionedRemote*` to `Runpod*`, update all imports in the same commit. If that diff becomes too large, leave type-prefix rename for a follow-up cleanup task but make serialized tags and variants RunPod-specific now.

- [ ] **Step 4: Update provision flow**

In `lifecycle/provision.rs`, replace provider lookup with `context.runpod_client`. Use:

```rust
let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
```

Create volume:

```rust
let network_volume_id = runpod_client
    .create_network_volume(CreateRunpodNetworkVolumeParams {
        workspace_id: workspace.id.clone(),
        data_center_id: runtime_state.placement.data_center_id.clone(),
        size_gb: runtime_state.placement.volume_size_gb,
    })
    .await?;
```

Persist:

```rust
runtime.resources.network_volume_id = Some(network_volume_id.clone());
```

Create endpoint:

```rust
let endpoint = runpod_client
    .create_serverless_endpoint(CreateRunpodEndpointParams {
        workspace_id: workspace.id.clone(),
        data_center_id: runtime.placement.data_center_id.clone(),
        gpu_type_id: runtime.placement.gpu_type_id.clone(),
        network_volume_id,
        endpoint_image_ref: contracts.endpoint_contract.image_ref.clone(),
        keep_alive_limits: runtime.placement.keep_alive_limits.clone(),
    })
    .await?;
```

Persist:

```rust
runtime.resources.endpoint_id = Some(endpoint.endpoint_id);
runtime.resources.template_id = Some(endpoint.template_id);
```

- [ ] **Step 5: Update cleanup/delete flow**

In both `cleanup.rs` and `delete.rs`, delete resources in this order:

```rust
if let Some(endpoint_id) = runtime.resources.endpoint_id.clone() {
    runpod_client.delete_serverless_endpoint(&endpoint_id).await?;
    runtime.resources.endpoint_id = None;
}

if let Some(template_id) = runtime.resources.template_id.clone() {
    runpod_client.delete_template(&template_id).await?;
    runtime.resources.template_id = None;
}

if let Some(provisioner_pod_id) = runtime.resources.provisioner_pod_id.clone() {
    runpod_client.terminate_provisioner_pod(&provisioner_pod_id).await?;
    runtime.resources.provisioner_pod_id = None;
}

if let Some(network_volume_id) = runtime.resources.network_volume_id.clone() {
    runpod_client.delete_network_volume(&network_volume_id).await?;
    runtime.resources.network_volume_id = None;
}
```

Handle not-found errors as success for the matching resource. Before deleting endpoint, reject corrupt runtime state:

```rust
if runtime.resources.endpoint_id.is_some() && runtime.resources.template_id.is_none() {
    return Err(ProvisionedRemoteError::InvalidRuntimeState);
}
```

- [ ] **Step 6: Update lifecycle payload serialization tests**

In `src-tauri/src/lifecycle_journal/payloads/provisioned_remote.rs`, change:

```rust
pub const RUNTIME_TYPE: &str = "runpod";
```

Update tests that assert `"runtime_type": "provisioned_remote"` to `"runtime_type": "runpod"`.

- [ ] **Step 7: Run lifecycle tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml provisioned_remote::service::tests lifecycle_journal::sqlite::tests domain::lifecycle_operation::tests
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/domain/provisioned_remote src-tauri/src/domain/lifecycle_operation.rs src-tauri/src/lifecycle_journal src-tauri/src/provisioned_remote/lifecycle src-tauri/src/provisioned_remote/errors.rs src-tauri/src/commands/mod.rs src-tauri/src/provisioned_remote/service.rs
git commit -m "refactor(runpod): use runpod lifecycle resources"
```

---

### Task 6: Command DTOs, Generated Bindings, And Frontend Diagnostics

**Files:**
- Modify: `src-tauri/src/commands/catalog.rs`
- Modify: `src-tauri/src/commands/workspaces.rs`
- Modify: `src-tauri/src/commands/types/{catalog,placement,provider,workspace}.rs`
- Modify: `src-tauri/src/lib.rs`
- Generated: `src/generated/commands.ts`
- Modify: `src/pages/home/ui/home-page.tsx`

- [ ] **Step 1: Write failing DTO serialization tests**

In `src-tauri/src/commands/types/placement.rs`, update or add:

```rust
#[test]
fn runpod_placement_plan_input_serializes_gb_fields() {
    let input = RunpodPlacementPlanInput {
        data_center_id: "EU-RO-1".to_string(),
        gpu_type_id: "NVIDIA RTX A5000".to_string(),
        volume_size_gb: 19,
        keep_alive_limits: None,
    };

    let json = serde_json::to_value(input).expect("placement json");

    assert_eq!(json["dataCenterId"], "EU-RO-1");
    assert_eq!(json["gpuTypeId"], "NVIDIA RTX A5000");
    assert_eq!(json["volumeSizeGb"], 19);
    assert!(json.get("gpuCloudProviderId").is_none());
    assert!(json.get("volumeSizeBytes").is_none());
}
```

In `commands/types/workspace.rs`, update create request serialization:

```rust
assert_eq!(json["placement"]["volumeSizeGb"], 19);
assert!(json.get("remotePlacement").is_none());
```

- [ ] **Step 2: Run focused failing command type tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml commands::types::placement::tests commands::types::workspace::tests
```

Expected: FAIL because DTO names and fields are still generic.

- [ ] **Step 3: Replace placement DTOs**

In `commands/types/placement.rs`, replace remote DTOs with:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunpodGpuPlacementOptionResponse {
    pub id: String,
    pub name: String,
    pub vram_gb: u64,
    pub availability_score: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunpodPlacementOptionsResponse {
    pub max_network_volume_size_gb: Option<u64>,
    pub datacenters: Vec<RunpodDatacenterPlacementOptionResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunpodPlacementPlanInput {
    pub data_center_id: String,
    pub gpu_type_id: String,
    pub volume_size_gb: u64,
    pub keep_alive_limits: Option<RunpodEndpointKeepAliveLimitsDto>,
}
```

Implement `From` conversions between DTOs and domain RunPod placement types.

- [ ] **Step 4: Replace catalog DTOs**

In `commands/types/catalog.rs`:

- delete `GetProviderPlacementOptionsRequest`
- delete `RemoteRuntimeRequirementsResponse`
- delete `RemoteProviderRuntimeRequirementsResponse`
- add `RunpodRuntimeRequirementsResponse`
- put `required_volume_size_gb` and `runpod_runtime_requirements` directly on `WorkflowRevisionResponse`

Use:

```rust
pub struct WorkflowRevisionResponse {
    pub version: String,
    pub requires_hugging_face_api_key: bool,
    pub required_volume_size_gb: u64,
    pub runpod_runtime_requirements: RunpodRuntimeRequirementsResponse,
    pub required_model_assets: Vec<ModelAssetResponse>,
}
```

- [ ] **Step 5: Rename Tauri commands**

In `commands/catalog.rs`, replace:

```rust
pub async fn get_provider_placement_options(...)
```

with:

```rust
pub async fn get_runpod_placement_options(
    state: State<'_, AppState>,
) -> CommandResult<RunpodPlacementOptionsResponse> {
    let options = state.runpod_runtime.get_runpod_placement_options().await?;
    Ok(options.into())
}
```

In `commands/workspaces.rs`, replace `create_workspace` with:

```rust
pub async fn create_runpod_workspace(
    state: State<'_, AppState>,
    request: CreateRunpodWorkspaceRequest,
) -> CommandResult<WorkspaceResponse> {
    let placement: RunpodPlacementPlan = request.placement.into();

    let workspace = state
        .runpod_runtime
        .create_runpod_workspace(CreateRunpodWorkspaceRequest {
            workspace_id: Uuid::new_v4().to_string(),
            workflow_preset_id: request.workflow_preset_id,
            placement,
        })
        .await?;

    Ok(workspace.into())
}
```

If the command DTO and service request share a name, alias one import to avoid ambiguity.

- [ ] **Step 6: Update command exports**

In `src-tauri/src/lib.rs`, replace command registrations:

```rust
commands::catalog::get_runpod_placement_options,
commands::workspaces::create_runpod_workspace,
```

Remove `commands::types::provider` from module exports if it only contained `GpuCloudProviderIdDto`.

- [ ] **Step 7: Regenerate command bindings**

Run:

```bash
bun run codegen:commands
```

Expected: PASS and `src/generated/commands.ts` contains `createRunpodWorkspace`, `getRunpodPlacementOptions`, `RunpodPlacementPlanInput`, and no `GpuCloudProviderIdDto`.

- [ ] **Step 8: Update Home diagnostics page**

In `src/pages/home/ui/home-page.tsx`:

- replace `GetProviderPlacementOptionsRequest` import with no request type
- replace command probe id/label for placement options
- call `commands.getRunpodPlacementOptions`
- replace `commands.createWorkspace` with `commands.createRunpodWorkspace`
- update initial create input:

```ts
initialInput: stringifyJson({
  workflowPresetId: "",
  placement: {
    dataCenterId: "",
    gpuTypeId: "",
    volumeSizeGb: 0,
    keepAliveLimits: null,
  },
}),
```

- [ ] **Step 9: Run command/frontend checks**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml commands::types
bun run build
bun run lint
```

Expected: PASS, except if `bun run lint` reports unrelated existing issues. If lint fails, record exact unrelated files and do not fix unrelated lint drift in this task.

- [ ] **Step 10: Commit**

```bash
git add src-tauri/src/commands src-tauri/src/lib.rs src/generated/commands.ts src/pages/home/ui/home-page.tsx
git commit -m "refactor(runpod): expose runpod command contract"
```

---

### Task 7: Final Rename Cleanup And Verification

**Files:**
- Modify or rename remaining `src-tauri/src/provisioned_remote/*` symbols if earlier tasks left compatibility names.
- Modify tests/docs only where they refer to removed names.

- [ ] **Step 1: Search for removed generic concepts**

Run:

```bash
rg -n "GpuCloudProviderId|GpuCloudProviderIdDto|provider_requirements|remote_runtime_requirements|required_base_volume_size|volume_size_bytes|vram_bytes|gpu_cloud_provider_id|get_provider_placement_options|create_workspace|ProvisionedRemoteProviderRegistry|ProviderAdapterUnavailable" src-tauri/src src bundled
```

Expected: no matches for removed concepts. Matches in old docs under `docs/superpowers` are acceptable only if they describe historical specs; do not edit unrelated historical docs.

- [ ] **Step 2: Rename module paths if still useful**

If the implementation still compiles under `src-tauri/src/provisioned_remote`, decide whether to do the physical module rename now. If doing it now, rename:

```text
src-tauri/src/provisioned_remote -> src-tauri/src/runpod_runtime
src-tauri/src/domain/provisioned_remote -> src-tauri/src/domain/runpod_runtime
src-tauri/src/lifecycle_journal/payloads/provisioned_remote.rs -> src-tauri/src/lifecycle_journal/payloads/runpod.rs
src-tauri/src/workspace_catalog/runtimes/provisioned_remote.rs -> src-tauri/src/workspace_catalog/runtimes/runpod.rs
```

Then update `mod.rs` exports and imports. Keep this as a pure rename/import cleanup commit; do not change behavior in the same commit.

- [ ] **Step 3: Run full native verification**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: PASS.

- [ ] **Step 4: Run full frontend verification**

Run:

```bash
bun run codegen:commands
bun run build
bun run lint
```

Expected: PASS. If lint fails from unrelated existing files, capture the exact output and keep this branch's files clean.

- [ ] **Step 5: Commit cleanup**

If Step 2 changed files:

```bash
git add src-tauri/src
git commit -m "refactor(runpod): rename runtime modules"
```

If only verification was performed and no files changed, do not create an empty commit.

---

## Self-Review Notes

- Spec coverage: Tasks cover workflow catalog requirements, command rename to `create_runpod_workspace`, `placement` request field, GB units, `RunpodRuntime`, `template_id` persistence, provider registry removal, lifecycle cleanup, generated commands, and frontend diagnostics.
- No compatibility path is planned for old `provider_id`, `provisioned_remote`, byte volume fields, or provider requirements.
- Type consistency: command DTO field is `placement`, service request field is `placement`, and domain placement type is `RunpodPlacementPlan`.
- Execution caution: because `docs/superpowers/*` is ignored locally, plan/spec commits in this area require `git add -f`.
