# RunPod Provider Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the RunPod remote workspace provider with handwritten typed REST/GraphQL adapters, secret-backed provisioner token derivation, and UI-safe provisioning error mapping.

**Architecture:** Keep orchestration in `RemoteWorkspaceService`; add a RunPod adapter under `src-tauri/src/remote_workspace/providers/runpod/` that owns all RunPod HTTP details and returns existing domain snapshots. No RunPod codegen is used: RunPod GraphQL introspection is disabled in production, and OpenAPI generation adds too much local tooling risk for the small provider boundary.

**Tech Stack:** Rust 2021, Tauri native backend, `reqwest`, `serde`, `serde_json`, `hmac`, `sha2`, `hex`, existing `AppFuture` async trait pattern, Rust unit tests.

---

## File Structure

- Modify `src-tauri/README.md`: update the provider path only.
- Modify `src-tauri/Cargo.toml`: add `hmac`, `sha2`, and `hex`.
- Modify `src-tauri/src/secrets_storage/service.rs`: add `hmac_sha256_hex`.
- Modify `src-tauri/src/remote_workspace/provider.rs`: add `requires_hugging_face_api_key` to `StartProvisionerParams`.
- Modify `src-tauri/src/remote_workspace/service.rs`: pass the workflow preset HF flag and preserve provisioner worker errors in `RemoteProvisioningStatus::Failed`.
- Modify `src-tauri/src/remote_workspace/errors.rs`: add `RemoteWorkspaceError::ProvisionerWorker`.
- Modify `src-tauri/src/remote_workspace/mod.rs`: expose `providers`.
- Create `src-tauri/src/remote_workspace/providers/mod.rs`: provider namespace.
- Create `src-tauri/src/remote_workspace/providers/runpod/mod.rs`: `RunpodRemoteWorkspaceProvider` and trait implementations.
- Create `src-tauri/src/remote_workspace/providers/runpod/config.rs`: constants and provider config.
- Create `src-tauri/src/remote_workspace/providers/runpod/api.rs`: handwritten typed RunPod REST and GraphQL HTTP wrapper.
- Create `src-tauri/src/remote_workspace/providers/runpod/mapping.rs`: RunPod and worker response mapping.
- Create `src-tauri/src/remote_workspace/providers/runpod/provisioner_worker.rs`: provisioner worker HTTP client.

No files are created under `src-tauri/src/generated`. No `generate:runpod` command, OpenAPI Generator config, GraphQL schema file, or `.runpod.graphql` operation file is added.

---

### Task 1: Add Secret-Backed HMAC Token Derivation

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/secrets_storage/service.rs`

- [ ] **Step 1: Add HMAC dependencies**

In `src-tauri/Cargo.toml`, add:

```toml
hex = "0.4"
hmac = "0.12"
sha2 = "0.10"
```

- [ ] **Step 2: Add failing HMAC tests**

In `src-tauri/src/secrets_storage/service.rs`, add tests:

```rust
#[tokio::test]
async fn hmac_sha256_hex_returns_lowercase_hex_digest() {
    let store = FakeStore::default();
    store.insert(SecretKey::RunpodApiKey, secret("secret"));
    let identity = FakeIdentityProvider::new(vec![]);
    let service = SecretsStorageService::new(store.clone(), identity, SecretKey::RunpodApiKey);

    let digest = service
        .hmac_sha256_hex("workspace-1")
        .await
        .expect("digest should be returned");

    assert_eq!(
        digest,
        "d2e1caef76d5f02c9335fcb4ce78501aa166c4fe16595e81a20f26d7ccc7500f"
    );
    assert_eq!(digest.len(), 64);
    assert!(digest.chars().all(|character| character.is_ascii_hexdigit()));
    assert_eq!(
        store.calls(),
        vec![StoreCall::Read(SecretKey::RunpodApiKey)]
    );
}

#[tokio::test]
async fn hmac_sha256_hex_returns_key_not_found_when_secret_missing() {
    let store = FakeStore::default();
    let identity = FakeIdentityProvider::new(vec![]);
    let service = SecretsStorageService::new(store, identity, SecretKey::RunpodApiKey);

    let result = service
        .hmac_sha256_hex("workspace-1")
        .await;

    assert_eq!(result, Err(SecretsStorageError::KeyNotFound));
}
```

- [ ] **Step 3: Run tests and verify failure**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml secrets_storage
```

Expected: FAIL because `hmac_sha256_hex` is not implemented.

- [ ] **Step 4: Implement HMAC method**

In `src-tauri/src/secrets_storage/service.rs`, add imports:

```rust
use hmac::{Hmac, Mac};
use sha2::Sha256;
```

Add this method inside the `impl<S, I> SecretsStorageService<S, I> where S: SecretStore, I: ApiKeyIdentityProvider` block:

```rust
pub async fn hmac_sha256_hex(
    &self,
    message: &str,
) -> Result<String, SecretsStorageError> {
    let secret = self
        .store
        .read(self.key)
        .await?
        .ok_or(SecretsStorageError::KeyNotFound)?;
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.expose_secret().as_bytes())
        .map_err(|_| SecretsStorageError::StoreUnavailable)?;

    mac.update(message.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}
```

- [ ] **Step 5: Run tests and commit**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml secrets_storage
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/secrets_storage
git commit -m "feat(secrets): add hmac token derivation"
```

Expected: PASS.

---

### Task 2: Preserve Provisioner Worker Errors In Provisioning Status

**Files:**
- Modify: `src-tauri/src/remote_workspace/errors.rs`
- Modify: `src-tauri/src/remote_workspace/service.rs`
- Modify: `src-tauri/src/remote_workspace/provider.rs`

- [ ] **Step 1: Add provider parameter field**

In `src-tauri/src/remote_workspace/provider.rs`, add:

```rust
pub requires_hugging_face_api_key: bool,
```

to `StartProvisionerParams`.

- [ ] **Step 2: Add worker error carrier**

In `src-tauri/src/remote_workspace/errors.rs`, import `RemoteProvisioningError` and add this variant:

```rust
ProvisionerWorker(RemoteProvisioningError),
```

Update `From<RemoteWorkspaceError> for RemoteProvisioningError`:

```rust
RemoteWorkspaceError::ProvisionerWorker(error) => error,
```

- [ ] **Step 3: Add service-level failing test for worker error mapping**

In `src-tauri/src/remote_workspace/service.rs`, add a test near existing running-provisioner tests:

```rust
#[test]
fn running_provisioner_worker_error_is_recorded_as_worker_error() {
    let state = Arc::new(Mutex::new(ProviderState {
        provisioner_status_result: Some(Err(RemoteWorkspaceError::ProvisionerWorker(
            RemoteProvisioningError::ProvisionerWorkerUnauthorized,
        ))),
        ..ProviderState::default()
    }));
    let service = service_with_state(state);
    let mut workspace = remote_workspace_in_progress(
        RemoteProvisioningPhase::RunningRemoteProvisioner {
            status: RemoteProvisionerStatus::Running,
        },
    );
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_resources.remote_provisioner = Some(RemoteProvisionerSnapshot {
        id: "provisioner".to_string(),
        status_url: "https://provisioner.example/status".to_string(),
    });

    let result = block_on(service.provision_workspace(&workspace))
        .expect("worker error should be converted into failed workspace");

    let WorkspaceRuntime::Remote(remote) = result.runtime;
    assert_eq!(
        remote.remote_provisioning.status,
        RemoteProvisioningStatus::Failed {
            phase: Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                status: RemoteProvisionerStatus::Running,
            }),
            error: RemoteProvisioningError::ProvisionerWorkerUnauthorized,
        }
    );
}
```

- [ ] **Step 4: Pass HF requirement flag in service**

In `handle_starting_provisioner`, update `StartProvisionerParams` construction:

```rust
requires_hugging_face_api_key: workspace.workflow_preset.requires_hugging_face_api_key,
```

Update fake provider expectations that compare `StartProvisionerParams` to include the same field.

- [ ] **Step 5: Run tests and commit**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml remote_workspace::service
git add src-tauri/src/remote_workspace/provider.rs src-tauri/src/remote_workspace/errors.rs src-tauri/src/remote_workspace/service.rs
git commit -m "feat(remote-workspace): preserve provisioner worker errors"
```

Expected: PASS.

---

### Task 3: Add RunPod Provider Module Skeleton And Config

**Files:**
- Modify: `src-tauri/README.md`
- Modify: `src-tauri/src/remote_workspace/mod.rs`
- Create: `src-tauri/src/remote_workspace/providers/mod.rs`
- Create: `src-tauri/src/remote_workspace/providers/runpod/mod.rs`
- Create: `src-tauri/src/remote_workspace/providers/runpod/config.rs`
- Create: `src-tauri/src/remote_workspace/providers/runpod/mapping.rs`

- [ ] **Step 1: Update provider path docs**

In `src-tauri/README.md`, update the provider path bullet to:

```markdown
2. Add a provider-specific module under `src/remote_workspace/providers/<provider_name>/`.
```

Do not add provider generation commands.

- [ ] **Step 2: Wire provider modules**

In `src-tauri/src/remote_workspace/mod.rs`, add:

```rust
pub mod providers;
```

Create `src-tauri/src/remote_workspace/providers/mod.rs`:

```rust
pub mod runpod;
```

- [ ] **Step 3: Add RunPod config constants**

Create `src-tauri/src/remote_workspace/providers/runpod/config.rs`:

```rust
use crate::domain::placement::RemoteEndpointKeepAliveLimits;

pub const RUNPOD_REST_BASE_URL: &str = "https://rest.runpod.io/v1";
pub const RUNPOD_GRAPHQL_URL: &str = "https://api.runpod.io/graphql";
pub const NETWORK_VOLUME_MAX_SIZE_BYTES: u64 = 4_000 * 1_000_000_000;
pub const WORKSPACE_MOUNT_PATH: &str = "/workspace";
pub const PROVISIONER_PORT: u16 = 8000;
pub const DEFAULT_ENDPOINT_KEEP_ALIVE_LIMITS: RemoteEndpointKeepAliveLimits =
    RemoteEndpointKeepAliveLimits {
        default_seconds: 300,
        min_seconds: 0,
        max_seconds: 86_400,
    };
```

- [ ] **Step 4: Add skeleton provider type with explicit stubs**

Create `src-tauri/src/remote_workspace/providers/runpod/mod.rs` with `RunpodRemoteWorkspaceProvider`, `provider_id`, and all remote provider trait methods returning `mapping::not_implemented("operation")`.

Create `src-tauri/src/remote_workspace/providers/runpod/mapping.rs`:

```rust
use crate::{
    domain::provider::ProviderApiError,
    remote_workspace::errors::RemoteWorkspaceError,
};

pub fn not_implemented(operation: &str) -> RemoteWorkspaceError {
    RemoteWorkspaceError::Provider(ProviderApiError::RequestFailed {
        message: format!("RunPod provider operation is not implemented: {operation}"),
    })
}
```

- [ ] **Step 5: Run compile and commit**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-run
git add src-tauri/README.md src-tauri/src/remote_workspace
git commit -m "feat(runpod): add provider module skeleton"
```

Expected: PASS.

---

### Task 4: Add Handwritten Typed RunPod API Wrapper

**Files:**
- Create: `src-tauri/src/remote_workspace/providers/runpod/api.rs`
- Modify: `src-tauri/src/remote_workspace/providers/runpod/mod.rs`
- Modify: `src-tauri/src/remote_workspace/providers/runpod/mapping.rs`

- [ ] **Step 1: Add local request/response structs**

Create `src-tauri/src/remote_workspace/providers/runpod/api.rs` with local `serde` structs for:

- `GraphqlRequest<V>`
- `GraphqlResponse<T>`
- placement query response types
- network volume create/delete responses
- pod create/delete responses
- template create/delete responses
- endpoint create/get/delete responses

Keep these structs private unless tests need `pub(super)`.

- [ ] **Step 2: Add provider-sized wrapper structs**

In `api.rs`, add:

```rust
use crate::domain::placement::RemoteEndpointKeepAliveLimits;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateNetworkVolumeRequest {
    pub datacenter_id: String,
    pub name: String,
    pub size_gb: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateProvisionerPodRequest {
    pub datacenter_id: String,
    pub image_ref: String,
    pub network_volume_id: String,
    pub mount_path: String,
    pub bearer_token: String,
    pub hugging_face_api_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateEndpointRequest {
    pub datacenter_id: String,
    pub gpu_id: String,
    pub image_ref: String,
    pub network_volume_id: String,
    pub mount_path: String,
    pub keep_alive_limits: RemoteEndpointKeepAliveLimits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodId {
    pub id: String,
}
```

- [ ] **Step 3: Add conversion helpers and tests**

In `mapping.rs`, add:

```rust
pub fn bytes_to_runpod_volume_gb(size_bytes: u64) -> u64 {
    size_bytes.div_ceil(1_000_000_000)
}

pub fn workspace_resource_name(workspace_id: &str, suffix: &str) -> String {
    format!("luma-forge-{workspace_id}-{suffix}")
}
```

Add tests for byte rounding and deterministic names.

- [ ] **Step 4: Add `RunpodApi` trait and `HttpRunpodApi`**

In `api.rs`, add a `RunpodApi` trait with methods for placement options, create/delete volume, create/delete provisioner pod, create endpoint, and delete endpoint plus template.

Add `HttpRunpodApi { http: reqwest::Client, rest_base_url: String, graphql_url: String, secrets: Arc<SecretsStorageService<..>> }`. It retrieves the key-scoped RunPod API key internally, sends typed `reqwest` requests, and maps responses to provider-sized structs and `RemoteWorkspaceError`.

- [ ] **Step 5: Add request serialization tests**

Add unit tests proving:

- network volume create serializes `dataCenterId`, name, and GB size
- provisioner pod create serializes CPU compute, volume mount, port, and env
- endpoint creation serializes template-before-endpoint request bodies
- placement GraphQL request uses a constant query string and variables-free JSON payload
- 401, 403, 429, timeout, operation-specific 404, and generic failures map to UI-safe errors

No tests make live RunPod calls.

- [ ] **Step 6: Run tests and commit**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml runpod::api
cargo test --manifest-path src-tauri/Cargo.toml runpod::mapping
git add src-tauri/src/remote_workspace/providers/runpod
git commit -m "feat(runpod): add typed api wrapper"
```

Expected: PASS.

---

### Task 5: Implement Provisioner Worker HTTP Client

**Files:**
- Create: `src-tauri/src/remote_workspace/providers/runpod/provisioner_worker.rs`
- Modify: `src-tauri/src/remote_workspace/providers/runpod/mod.rs`

- [ ] **Step 1: Add worker response mapping**

Create `src-tauri/src/remote_workspace/providers/runpod/provisioner_worker.rs` with `ProvisionerStatusResponse`, `ProvisionerWorkerErrorResponse`, and `map_status_response`.

Map:

- `idle` to `RemoteProvisionerStatus::Pending`
- `running` to `RemoteProvisionerStatus::Running`
- `succeeded` to `RemoteProvisionerStatus::Succeeded`
- `failed` with worker error to `RemoteProvisionerStatus::Failed { code, message }`
- malformed responses to `RemoteProvisioningError::ProvisionerWorkerResponseInvalid`

- [ ] **Step 2: Add HTTP method**

Add `ProvisionerWorkerClient { http: reqwest::Client }` with:

```rust
pub async fn get_status(
    &self,
    status_url: &str,
    bearer_token: &str,
) -> Result<RemoteProvisionerStatus, RemoteProvisioningError>
```

Map connection failures to `ProvisionerWorkerUnavailable`, worker `401` to `ProvisionerWorkerUnauthorized`, worker `409` to `ProvisionerWorkerConflict`, invalid JSON to `ProvisionerWorkerResponseInvalid`, and other statuses to `ProvisionerWorkerUnexpectedError`.

- [ ] **Step 3: Add tests and commit**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml runpod::provisioner_worker
git add src-tauri/src/remote_workspace/providers/runpod
git commit -m "feat(runpod): add provisioner worker client"
```

Expected: PASS.

---

### Task 6: Implement RunPod Provider Trait Methods

**Files:**
- Modify: `src-tauri/src/remote_workspace/providers/runpod/mod.rs`
- Modify: `src-tauri/src/remote_workspace/providers/runpod/api.rs`
- Modify: `src-tauri/src/remote_workspace/providers/runpod/config.rs`

- [ ] **Step 1: Replace skeleton with provider-owned default clients**

Change `RunpodRemoteWorkspaceProvider::new` to accept:

- already-initialized `SecretsStorageService` for RunPod
- already-initialized `SecretsStorageService` for Hugging Face

The provider constructs its default `HttpRunpodApi` and `ProvisionerWorkerClient` internally from provider constants. Test-only constructors may inject fake API and worker clients. Do not initialize secret stores inside the provider.

- [ ] **Step 2: Implement placement and volume traits**

Behavior:

- placement delegates to `api.placement_options()`
- placement sets `max_persistent_storage_volume_size_bytes` to `NETWORK_VOLUME_MAX_SIZE_BYTES`
- create volume builds `CreateNetworkVolumeRequest`
- delete volume delegates to `api.delete_network_volume`

- [ ] **Step 3: Implement provisioner traits**

Behavior:

- derive token with `runpod_secrets.hmac_sha256_hex(&params.workspace_id)`
- retrieve HF key only when `params.requires_hugging_face_api_key` is `true`
- create pod through `api.create_provisioner_pod`
- return `RemoteProvisionerSnapshot { id, status_url }`
- get status derives the same token and calls `ProvisionerWorkerClient`
- worker errors become `RemoteWorkspaceError::ProvisionerWorker(error)`

- [ ] **Step 4: Implement endpoint traits**

Behavior:

- endpoint creation resolves `params.keep_alive_limits.unwrap_or(DEFAULT_ENDPOINT_KEEP_ALIVE_LIMITS)`
- endpoint creation calls API template-before-endpoint wrapper
- endpoint deletion delegates to `api.delete_endpoint_and_template`

- [ ] **Step 5: Add provider tests**

Add tests named:

```rust
create_volume_builds_network_volume_request
start_provisioner_derives_token_and_injects_hf_when_required
start_provisioner_omits_hf_when_not_required
get_provisioner_status_maps_worker_unauthorized_to_workspace_worker_error
create_endpoint_uses_default_keep_alive_limits_when_missing
delete_endpoint_delegates_endpoint_and_template_cleanup
```

- [ ] **Step 6: Run tests and commit**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml runpod
git add src-tauri/src/remote_workspace/providers/runpod
git commit -m "feat(runpod): implement provider traits"
```

Expected: PASS.

---

### Task 7: Register Provider And Final Verification

**Files:**
- Modify: `src-tauri/src/remote_workspace/registry.rs`
- Modify: `src-tauri/src/remote_workspace/providers/runpod/mod.rs`

- [ ] **Step 1: Add registration constructor path**

Add:

```rust
pub fn with_provider(provider: Box<dyn RemoteWorkspaceProvider>) -> Self {
    Self {
        providers: vec![provider],
    }
}
```

Add a registry test proving a boxed `RunpodRemoteWorkspaceProvider` resolves to `GpuCloudProviderId::Runpod` using fake dependencies.

- [ ] **Step 2: Run native verification**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: all commands pass.

- [ ] **Step 3: Commit**

Run:

```bash
git add src-tauri/src/remote_workspace src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat(runpod): register remote workspace provider"
```

---

## Self-Review

- Spec coverage: no-codegen decision, provider module layout, typed REST/GraphQL wrappers, secret-service injection, HMAC derivation, HF flag propagation, placement max volume, RunPod REST lifecycle, provisioner worker status/error mapping, default keep-alive limits, and verification are covered by tasks.
- Placeholder scan: the plan contains no deferred sections, no deferred error handling, and no live RunPod tests in the default path.
- Type consistency: `hmac_sha256_hex`, `StartProvisionerParams.requires_hugging_face_api_key`, `RemoteWorkspaceError::ProvisionerWorker`, and provider module paths are introduced before later tasks use them.
