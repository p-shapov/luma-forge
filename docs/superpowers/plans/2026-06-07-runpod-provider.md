# RunPod Provider Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the RunPod remote workspace provider with generated REST and GraphQL clients, secret-backed provisioner token derivation, and UI-safe provisioning error mapping.

**Architecture:** Keep orchestration in `RemoteWorkspaceService`; add a RunPod adapter under `src-tauri/src/remote_workspace/providers/runpod/` that wraps generated RunPod API types and returns existing domain snapshots. Generated code stays under `src-tauri/src/generated`, while maintained GraphQL operations live beside the provider and are discovered by the `*.runpod.graphql` suffix.

**Tech Stack:** Rust 2021, Tauri native backend, `reqwest`, `serde`, `graphql_client_cli`, OpenAPI Generator Rust client, `hmac`, `sha2`, `hex`, existing `AppFuture` async trait pattern, Rust unit tests.

---

## File Structure

- Create `scripts/generate-runpod.mjs`: provider-scoped generation script for RunPod REST and GraphQL artifacts.
- Modify `package.json`: add `generate:runpod`.
- Modify `src-tauri/README.md`: document provider path and `generate:runpod`.
- Modify `src-tauri/Cargo.toml`: add HMAC and GraphQL dependencies.
- Create `src-tauri/src/generated/mod.rs`: generated module entrypoint.
- Create `src-tauri/src/generated/runpod_graphql/`: generated GraphQL schema and Rust modules.
- Create `src-tauri/src/generated/runpod_rest/`: generated OpenAPI Rust client.
- Modify `src-tauri/src/lib.rs`: expose `generated`.
- Modify `src-tauri/src/secrets_storage/store.rs`: add `SecretKey::ProvisionerTokenSecret`.
- Modify `src-tauri/src/secrets_storage/service.rs`: add `hmac_sha256_hex`.
- Modify `src-tauri/src/remote_workspace/provider.rs`: add `requires_hugging_face_api_key` to `StartProvisionerParams`.
- Modify `src-tauri/src/remote_workspace/service.rs`: pass the workflow preset HF flag and preserve provisioner worker errors in `RemoteProvisioningStatus::Failed`.
- Modify `src-tauri/src/remote_workspace/errors.rs`: add `RemoteWorkspaceError::ProvisionerWorker`.
- Modify `src-tauri/src/remote_workspace/mod.rs`: expose `providers`.
- Create `src-tauri/src/remote_workspace/providers/mod.rs`: provider namespace.
- Create `src-tauri/src/remote_workspace/providers/runpod/mod.rs`: `RunpodRemoteWorkspaceProvider` and trait implementations.
- Create `src-tauri/src/remote_workspace/providers/runpod/config.rs`: constants and provider config.
- Create `src-tauri/src/remote_workspace/providers/runpod/api.rs`: generated REST/GraphQL wrapper.
- Create `src-tauri/src/remote_workspace/providers/runpod/mapping.rs`: RunPod and worker response mapping.
- Create `src-tauri/src/remote_workspace/providers/runpod/provisioner_worker.rs`: provisioner worker HTTP client.
- Create `src-tauri/src/remote_workspace/providers/runpod/graphql/placement_options.runpod.graphql`: RunPod placement query.

---

### Task 1: Add RunPod Generation Infrastructure

**Files:**
- Create: `scripts/generate-runpod.mjs`
- Modify: `package.json`
- Modify: `src-tauri/README.md`
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/generated/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `src-tauri/src/remote_workspace/providers/runpod/graphql/placement_options.runpod.graphql`

- [ ] **Step 1: Add the RunPod GraphQL operation source**

Create `src-tauri/src/remote_workspace/providers/runpod/graphql/placement_options.runpod.graphql`:

```graphql
query PlacementOptions {
  dataCenters {
    id
    name
    listed
    storageSupport
  }
  gpuTypes {
    id
    displayName
    memoryInGb
  }
  gpuTypesDatacenters {
    dataCenterId
    gpuTypeId
    available
    secureCloud
    communityCloud
  }
}
```

- [ ] **Step 2: Add generated module entrypoint**

Create `src-tauri/src/generated/mod.rs`:

```rust
pub mod runpod_graphql;
pub mod runpod_rest;
```

Modify `src-tauri/src/lib.rs` to add:

```rust
pub mod generated;
```

- [ ] **Step 3: Add generation dependencies**

In `src-tauri/Cargo.toml`, add these dependencies:

```toml
graphql_client = "0.16"
hmac = "0.12"
sha2 = "0.10"
hex = "0.4"
```

- [ ] **Step 4: Add `generate:runpod` script**

Add this script to `package.json`:

```json
"generate:runpod": "bun scripts/generate-runpod.mjs"
```

Create `scripts/generate-runpod.mjs`:

```javascript
import { mkdirSync, readdirSync, rmSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'

const root = dirname(fileURLToPath(import.meta.url)).replace(/\/scripts$/, '')
const generatedRoot = join(root, 'src-tauri/src/generated')
const restRoot = join(generatedRoot, 'runpod_rest')
const graphqlRoot = join(generatedRoot, 'runpod_graphql')
const graphqlOpsRoot = join(root, 'src-tauri/src/remote_workspace/providers/runpod/graphql')

function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, stdio: 'inherit' })
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(' ')} failed with status ${result.status}`)
  }
}

function graphqlOperations(dir) {
  return readdirSync(dir)
    .filter((name) => name.endsWith('.runpod.graphql'))
    .map((name) => join(dir, name))
}

rmSync(restRoot, { recursive: true, force: true })
rmSync(graphqlRoot, { recursive: true, force: true })
mkdirSync(restRoot, { recursive: true })
mkdirSync(graphqlRoot, { recursive: true })

const openapiPath = join(restRoot, 'openapi.json')
const schemaPath = join(graphqlRoot, 'schema.json')

run('curl', ['-fsSL', 'https://rest.runpod.io/v1/openapi.json', '-o', openapiPath])
run('graphql-client', ['introspect-schema', 'https://graphql-spec.runpod.io/', '--output', schemaPath])

run('bunx', [
  '@openapitools/openapi-generator-cli',
  'generate',
  '-i',
  openapiPath,
  '-g',
  'rust',
  '-o',
  restRoot,
  '--additional-properties=packageName=runpod_rest,library=reqwest,avoidBoxedModels=true',
])

for (const operation of graphqlOperations(graphqlOpsRoot)) {
  run('graphql-client', [
    'generate',
    '--schema-path',
    schemaPath,
    '--response-derives',
    'Debug,Clone,PartialEq',
    '--output-directory',
    graphqlRoot,
    operation,
  ])
}
```

- [ ] **Step 5: Document generation**

In `src-tauri/README.md`, update the provider path bullet to:

```markdown
2. Add a provider-specific module under `src/remote_workspace/providers/<provider_name>/`.
```

Add this section after "Adding A Remote Provider":

```markdown
### Generated Provider Clients

RunPod generated clients are regenerated with:

```bash
bun run generate:runpod
```

The command downloads the RunPod REST OpenAPI schema, introspects the RunPod GraphQL schema, generates Rust REST code under `src/generated/runpod_rest`, and generates GraphQL Rust bindings under `src/generated/runpod_graphql`.

RunPod GraphQL operation files are maintained source files matching:

```text
src/remote_workspace/providers/runpod/graphql/*.runpod.graphql
```

The generation command discovers RunPod operations by that suffix. Generated files are replaceable output; provider wrapper code belongs under `src/remote_workspace/providers/runpod/`.
```

- [ ] **Step 6: Run generation and verify compile errors are generation-related**

Run:

```bash
bun run generate:runpod
cargo test --manifest-path src-tauri/Cargo.toml --no-run
```

Expected: generation completes. If `cargo test --no-run` fails because generated OpenAPI files require module-path fixes, make only generated-module containment fixes under `src-tauri/src/generated` and rerun until compilation reaches handwritten code.

- [ ] **Step 7: Commit**

Run:

```bash
git add package.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/README.md scripts/generate-runpod.mjs src-tauri/src/generated src-tauri/src/lib.rs src-tauri/src/remote_workspace/providers/runpod/graphql/placement_options.runpod.graphql
git commit -m "chore(runpod): add provider client generation"
```

---

### Task 2: Add Secret-Backed HMAC Token Derivation

**Files:**
- Modify: `src-tauri/src/secrets_storage/store.rs`
- Modify: `src-tauri/src/secrets_storage/service.rs`

- [ ] **Step 1: Add failing secret key serialization test**

In `src-tauri/src/secrets_storage/store.rs`, extend `SecretKey`:

```rust
#[serde(rename = "provisioner-token")]
ProvisionerTokenSecret,
```

Then add this assertion to `secret_key_serializes_as_storage_account_identifier`:

```rust
assert_eq!(
    serde_json::to_string(&SecretKey::ProvisionerTokenSecret).expect("secret key json"),
    "\"provisioner-token\""
);
```

- [ ] **Step 2: Add failing HMAC tests**

In `src-tauri/src/secrets_storage/service.rs`, add tests:

```rust
#[tokio::test]
async fn hmac_sha256_hex_returns_lowercase_hex_digest() {
    let store = FakeStore::default();
    store.insert(SecretKey::ProvisionerTokenSecret, secret("secret"));
    let identity = FakeIdentityProvider::new(vec![]);
    let service = SecretsStorageService::new(store.clone(), identity);

    let digest = service
        .hmac_sha256_hex(SecretKey::ProvisionerTokenSecret, "workspace-1")
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
        vec![StoreCall::Read(SecretKey::ProvisionerTokenSecret)]
    );
}

#[tokio::test]
async fn hmac_sha256_hex_returns_key_not_found_when_secret_missing() {
    let store = FakeStore::default();
    let identity = FakeIdentityProvider::new(vec![]);
    let service = SecretsStorageService::new(store, identity);

    let result = service
        .hmac_sha256_hex(SecretKey::ProvisionerTokenSecret, "workspace-1")
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
    key: SecretKey,
    message: &str,
) -> Result<String, SecretsStorageError> {
    let secret = self
        .store
        .read(key)
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
git add src-tauri/src/secrets_storage src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat(secrets): add hmac token derivation"
```

Expected: PASS.

---

### Task 3: Preserve Provisioner Worker Errors In Provisioning Status

**Files:**
- Modify: `src-tauri/src/remote_workspace/errors.rs`
- Modify: `src-tauri/src/remote_workspace/service.rs`
- Modify: `src-tauri/src/remote_workspace/provider.rs`

- [ ] **Step 1: Add provider parameter field test pressure**

In `src-tauri/src/remote_workspace/provider.rs`, add:

```rust
pub requires_hugging_face_api_key: bool,
```

to `StartProvisionerParams`.

In `src-tauri/src/remote_workspace/service.rs` fake provider tests, update recorded `StartProvisionerParams` expectations to include:

```rust
requires_hugging_face_api_key: workspace.workflow_preset.requires_hugging_face_api_key,
```

- [ ] **Step 2: Add worker error carrier**

In `src-tauri/src/remote_workspace/errors.rs`, add this variant:

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
    let mut workspace = remote_workspace_in_progress(RemoteProvisioningPhase::RunningRemoteProvisioner {
        status: RemoteProvisionerStatus::Running,
    });
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

- [ ] **Step 5: Run tests and commit**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml remote_workspace::service
git add src-tauri/src/remote_workspace/provider.rs src-tauri/src/remote_workspace/errors.rs src-tauri/src/remote_workspace/service.rs
git commit -m "feat(remote-workspace): preserve provisioner worker errors"
```

Expected: PASS.

---

### Task 4: Add RunPod Provider Module Skeleton And Config

**Files:**
- Modify: `src-tauri/src/remote_workspace/mod.rs`
- Create: `src-tauri/src/remote_workspace/providers/mod.rs`
- Create: `src-tauri/src/remote_workspace/providers/runpod/mod.rs`
- Create: `src-tauri/src/remote_workspace/providers/runpod/config.rs`
- Create: `src-tauri/src/remote_workspace/providers/runpod/mapping.rs`

- [ ] **Step 1: Wire provider modules**

In `src-tauri/src/remote_workspace/mod.rs`, add:

```rust
pub mod providers;
```

Create `src-tauri/src/remote_workspace/providers/mod.rs`:

```rust
pub mod runpod;
```

- [ ] **Step 2: Add RunPod config constants**

Create `src-tauri/src/remote_workspace/providers/runpod/config.rs`:

```rust
use crate::domain::placement::RemoteEndpointKeepAliveLimits;

pub const RUNPOD_REST_BASE_URL: &str = "https://rest.runpod.io/v1";
pub const RUNPOD_GRAPHQL_URL: &str = "https://graphql-spec.runpod.io/";
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

- [ ] **Step 3: Add minimal provider type**

Create `src-tauri/src/remote_workspace/providers/runpod/mod.rs`:

```rust
mod config;
mod mapping;

use crate::domain::provider::GpuCloudProviderId;
use crate::domain::workspace::{
    RemoteEndpointSnapshot, RemoteProvisionerSnapshot, RemoteProvisionerStatus,
    RemoteVolumeSnapshot,
};
use crate::remote_workspace::errors::RemoteWorkspaceError;
use crate::remote_workspace::provider::{
    CreateEndpointParams, CreateVolumeParams, DeleteEndpointParams, DeleteVolumeParams,
    GetProvisionerStatusParams, RemoteEndpointProvider, RemotePlacementOptionsProvider,
    RemoteProvisionerProvider, RemoteVolumeProvider, RemoteWorkspaceProvider,
    StartProvisionerParams, TerminateProvisionerParams,
};
use crate::shared::AppFuture;

pub struct RunpodRemoteWorkspaceProvider;

impl RunpodRemoteWorkspaceProvider {
    pub fn new() -> Self {
        Self
    }
}

impl RemotePlacementOptionsProvider for RunpodRemoteWorkspaceProvider {
    fn get_provider_placement_options<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<crate::domain::placement::RemotePlacementOptions, RemoteWorkspaceError>>
    {
        Box::pin(async { Err(mapping::not_implemented("get_provider_placement_options")) })
    }
}

impl RemoteVolumeProvider for RunpodRemoteWorkspaceProvider {
    fn create_volume<'a>(
        &'a self,
        _params: CreateVolumeParams,
    ) -> AppFuture<'a, Result<RemoteVolumeSnapshot, RemoteWorkspaceError>> {
        Box::pin(async { Err(mapping::not_implemented("create_volume")) })
    }

    fn delete_volume<'a>(
        &'a self,
        _params: DeleteVolumeParams,
    ) -> AppFuture<'a, Result<(), RemoteWorkspaceError>> {
        Box::pin(async { Err(mapping::not_implemented("delete_volume")) })
    }
}

impl RemoteProvisionerProvider for RunpodRemoteWorkspaceProvider {
    fn start_provisioner<'a>(
        &'a self,
        _params: StartProvisionerParams,
    ) -> AppFuture<'a, Result<RemoteProvisionerSnapshot, RemoteWorkspaceError>> {
        Box::pin(async { Err(mapping::not_implemented("start_provisioner")) })
    }

    fn terminate_provisioner<'a>(
        &'a self,
        _params: TerminateProvisionerParams,
    ) -> AppFuture<'a, Result<(), RemoteWorkspaceError>> {
        Box::pin(async { Err(mapping::not_implemented("terminate_provisioner")) })
    }

    fn get_provisioner_status<'a>(
        &'a self,
        _params: GetProvisionerStatusParams,
    ) -> AppFuture<'a, Result<RemoteProvisionerStatus, RemoteWorkspaceError>> {
        Box::pin(async { Err(mapping::not_implemented("get_provisioner_status")) })
    }
}

impl RemoteEndpointProvider for RunpodRemoteWorkspaceProvider {
    fn create_endpoint<'a>(
        &'a self,
        _params: CreateEndpointParams,
    ) -> AppFuture<'a, Result<RemoteEndpointSnapshot, RemoteWorkspaceError>> {
        Box::pin(async { Err(mapping::not_implemented("create_endpoint")) })
    }

    fn delete_endpoint<'a>(
        &'a self,
        _params: DeleteEndpointParams,
    ) -> AppFuture<'a, Result<(), RemoteWorkspaceError>> {
        Box::pin(async { Err(mapping::not_implemented("delete_endpoint")) })
    }
}

impl RemoteWorkspaceProvider for RunpodRemoteWorkspaceProvider {
    fn provider_id(&self) -> GpuCloudProviderId {
        GpuCloudProviderId::Runpod
    }
}
```

Create `src-tauri/src/remote_workspace/providers/runpod/mapping.rs`:

```rust
use crate::remote_workspace::errors::RemoteWorkspaceError;

pub fn not_implemented(operation: &str) -> RemoteWorkspaceError {
    RemoteWorkspaceError::Provider(crate::domain::provider::ProviderApiError::RequestFailed {
        message: format!("RunPod provider operation is not implemented: {operation}"),
    })
}
```

- [ ] **Step 4: Run compile and commit**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-run
git add src-tauri/src/remote_workspace
git commit -m "feat(runpod): add provider module skeleton"
```

Expected: compile passes with all RunPod resource methods returning explicit UI-safe not-implemented provider errors.

---

### Task 5: Implement Provisioner Worker HTTP Client

**Files:**
- Create: `src-tauri/src/remote_workspace/providers/runpod/provisioner_worker.rs`
- Modify: `src-tauri/src/remote_workspace/providers/runpod/mod.rs`
- Modify: `src-tauri/src/remote_workspace/providers/runpod/mapping.rs`

- [ ] **Step 1: Add worker response types and mapping tests**

Create `src-tauri/src/remote_workspace/providers/runpod/provisioner_worker.rs` with tests:

```rust
use crate::domain::workspace::{RemoteProvisionerStatus, RemoteProvisioningError};

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ProvisionerStatusResponse {
    pub status: String,
    pub error: Option<ProvisionerWorkerErrorResponse>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ProvisionerWorkerErrorResponse {
    pub code: String,
    pub message: String,
}

pub fn map_status_response(
    response: ProvisionerStatusResponse,
) -> Result<RemoteProvisionerStatus, RemoteProvisioningError> {
    match response.status.as_str() {
        "idle" => Ok(RemoteProvisionerStatus::Pending),
        "running" => Ok(RemoteProvisionerStatus::Running),
        "succeeded" => Ok(RemoteProvisionerStatus::Succeeded),
        "failed" => {
            let error = response.error.ok_or(RemoteProvisioningError::ProvisionerWorkerResponseInvalid)?;
            Ok(RemoteProvisionerStatus::Failed {
                code: error.code,
                message: error.message,
            })
        }
        _ => Err(RemoteProvisioningError::ProvisionerWorkerResponseInvalid),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_idle_to_pending() {
        assert_eq!(
            map_status_response(ProvisionerStatusResponse {
                status: "idle".to_string(),
                error: None,
            }),
            Ok(RemoteProvisionerStatus::Pending)
        );
    }

    #[test]
    fn maps_failed_status_with_worker_error() {
        assert_eq!(
            map_status_response(ProvisionerStatusResponse {
                status: "failed".to_string(),
                error: Some(ProvisionerWorkerErrorResponse {
                    code: "asset_download_failed".to_string(),
                    message: "Hugging Face asset download failed".to_string(),
                }),
            }),
            Ok(RemoteProvisionerStatus::Failed {
                code: "asset_download_failed".to_string(),
                message: "Hugging Face asset download failed".to_string(),
            })
        );
    }
}
```

- [ ] **Step 2: Add HTTP method**

Add to the same file:

```rust
pub struct ProvisionerWorkerClient {
    http: reqwest::Client,
}

impl ProvisionerWorkerClient {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }

    pub async fn get_status(
        &self,
        status_url: &str,
        bearer_token: &str,
    ) -> Result<RemoteProvisionerStatus, RemoteProvisioningError> {
        let response = self
            .http
            .get(status_url)
            .bearer_auth(bearer_token)
            .send()
            .await
            .map_err(|_| RemoteProvisioningError::ProvisionerWorkerUnavailable)?;

        match response.status().as_u16() {
            200 => {
                let payload = response
                    .json::<ProvisionerStatusResponse>()
                    .await
                    .map_err(|_| RemoteProvisioningError::ProvisionerWorkerResponseInvalid)?;
                map_status_response(payload)
            }
            401 => Err(RemoteProvisioningError::ProvisionerWorkerUnauthorized),
            409 => Err(RemoteProvisioningError::ProvisionerWorkerConflict),
            _ => Err(RemoteProvisioningError::ProvisionerWorkerUnexpectedError),
        }
    }
}
```

- [ ] **Step 3: Convert worker errors into `RemoteWorkspaceError` at provider boundary**

In `src-tauri/src/remote_workspace/providers/runpod/mod.rs`, add:

```rust
mod provisioner_worker;
```

Provider methods that call `ProvisionerWorkerClient::get_status` must convert `Err(error)` to:

```rust
RemoteWorkspaceError::ProvisionerWorker(error)
```

- [ ] **Step 4: Run tests and commit**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml runpod::provisioner_worker
git add src-tauri/src/remote_workspace/providers/runpod
git commit -m "feat(runpod): add provisioner worker client"
```

Expected: PASS.

---

### Task 6: Implement RunPod API Wrapper And Request Mapping

**Files:**
- Create: `src-tauri/src/remote_workspace/providers/runpod/api.rs`
- Modify: `src-tauri/src/remote_workspace/providers/runpod/mod.rs`
- Modify: `src-tauri/src/remote_workspace/providers/runpod/mapping.rs`

- [ ] **Step 1: Add provider-sized request and response structs**

Create `src-tauri/src/remote_workspace/providers/runpod/api.rs`:

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

- [ ] **Step 2: Add conversion helpers and tests**

In `mapping.rs`, add:

```rust
pub fn bytes_to_runpod_volume_gb(size_bytes: u64) -> u64 {
    size_bytes.div_ceil(1_000_000_000)
}

pub fn workspace_resource_name(workspace_id: &str, suffix: &str) -> String {
    format!("luma-forge-{workspace_id}-{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_size_rounds_up_to_gb() {
        assert_eq!(bytes_to_runpod_volume_gb(1), 1);
        assert_eq!(bytes_to_runpod_volume_gb(1_000_000_000), 1);
        assert_eq!(bytes_to_runpod_volume_gb(1_000_000_001), 2);
    }

    #[test]
    fn workspace_resource_name_is_deterministic() {
        assert_eq!(
            workspace_resource_name("workspace-1", "volume"),
            "luma-forge-workspace-1-volume"
        );
    }
}
```

- [ ] **Step 3: Add generated-client wrapper trait**

In `api.rs`, add a trait the provider can use in tests without live RunPod calls:

```rust
use crate::domain::placement::RemotePlacementOptions;
use crate::remote_workspace::errors::RemoteWorkspaceError;
use crate::shared::AppFuture;

pub trait RunpodApi: Send + Sync {
    fn placement_options<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<RemotePlacementOptions, RemoteWorkspaceError>>;

    fn create_network_volume<'a>(
        &'a self,
        request: CreateNetworkVolumeRequest,
    ) -> AppFuture<'a, Result<RunpodId, RemoteWorkspaceError>>;

    fn delete_network_volume<'a>(
        &'a self,
        volume_id: &'a str,
    ) -> AppFuture<'a, Result<(), RemoteWorkspaceError>>;

    fn create_provisioner_pod<'a>(
        &'a self,
        request: CreateProvisionerPodRequest,
    ) -> AppFuture<'a, Result<(RunpodId, String), RemoteWorkspaceError>>;

    fn delete_pod<'a>(&'a self, pod_id: &'a str) -> AppFuture<'a, Result<(), RemoteWorkspaceError>>;

    fn create_endpoint<'a>(
        &'a self,
        request: CreateEndpointRequest,
    ) -> AppFuture<'a, Result<RunpodId, RemoteWorkspaceError>>;

    fn delete_endpoint_and_template<'a>(
        &'a self,
        endpoint_id: &'a str,
    ) -> AppFuture<'a, Result<(), RemoteWorkspaceError>>;
}
```

- [ ] **Step 4: Add generated REST implementation behind wrapper**

Implement `GeneratedRunpodApi` in `api.rs` using the generated REST and GraphQL modules. Keep all references to `crate::generated::runpod_rest` and `crate::generated::runpod_graphql` inside this file. Map `401`, `403`, `429`, request timeouts, operation-specific `404`, and other failures to `RemoteWorkspaceError` as defined in the design spec.

- [ ] **Step 5: Run mapping tests and commit**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml runpod::mapping
cargo test --manifest-path src-tauri/Cargo.toml --no-run
git add src-tauri/src/remote_workspace/providers/runpod
git commit -m "feat(runpod): add api wrapper mapping"
```

Expected: PASS.

---

### Task 7: Implement RunPod Provider Trait Methods

**Files:**
- Modify: `src-tauri/src/remote_workspace/providers/runpod/mod.rs`
- Modify: `src-tauri/src/remote_workspace/providers/runpod/api.rs`
- Modify: `src-tauri/src/remote_workspace/providers/runpod/config.rs`

- [ ] **Step 1: Replace skeleton with dependency-injected provider**

In `mod.rs`, change the provider struct to:

```rust
use std::sync::Arc;

use crate::secrets_storage::{ApiKeyIdentityProvider, SecretKey, SecretStore, SecretsStorageService};

use self::api::RunpodApi;
use self::provisioner_worker::ProvisionerWorkerClient;

pub struct RunpodRemoteWorkspaceProvider<R, H, P, RI, HI, PI>
where
    R: SecretStore,
    H: SecretStore,
    P: SecretStore,
    RI: ApiKeyIdentityProvider,
    HI: ApiKeyIdentityProvider,
    PI: ApiKeyIdentityProvider,
{
    api: Arc<dyn RunpodApi>,
    provisioner_worker: ProvisionerWorkerClient,
    runpod_secrets: SecretsStorageService<R, RI>,
    hugging_face_secrets: SecretsStorageService<H, HI>,
    provisioner_secrets: SecretsStorageService<P, PI>,
}
```

Add a `new(...)` constructor that accepts these dependencies. Do not initialize secret stores inside the provider.

- [ ] **Step 2: Implement placement and volume traits**

Implement:

```rust
RemotePlacementOptionsProvider for RunpodRemoteWorkspaceProvider<...>
RemoteVolumeProvider for RunpodRemoteWorkspaceProvider<...>
```

Behavior:

- placement delegates to `api.placement_options()`
- create volume builds `CreateNetworkVolumeRequest`
- delete volume delegates to `api.delete_network_volume`

- [ ] **Step 3: Implement provisioner traits**

Implement:

```rust
RemoteProvisionerProvider for RunpodRemoteWorkspaceProvider<...>
```

Behavior:

- derive token with `provisioner_secrets.hmac_sha256_hex(SecretKey::ProvisionerTokenSecret, &params.workspace_id)`
- retrieve HF key only when `params.requires_hugging_face_api_key` is `true`
- create pod through `api.create_provisioner_pod`
- return `RemoteProvisionerSnapshot { id, status_url }`
- get status derives the same token and calls `ProvisionerWorkerClient`
- worker errors become `RemoteWorkspaceError::ProvisionerWorker(error)`

- [ ] **Step 4: Implement endpoint traits**

Implement:

```rust
RemoteEndpointProvider for RunpodRemoteWorkspaceProvider<...>
```

Behavior:

- endpoint creation resolves `params.keep_alive_limits.unwrap_or(DEFAULT_ENDPOINT_KEEP_ALIVE_LIMITS)`
- endpoint creation calls API template-before-endpoint wrapper
- endpoint deletion delegates to `api.delete_endpoint_and_template`

- [ ] **Step 5: Add provider unit tests with fake API and fake stores**

In `mod.rs`, add tests named:

```rust
create_volume_builds_network_volume_request
start_provisioner_derives_token_and_injects_hf_when_required
start_provisioner_omits_hf_when_not_required
get_provisioner_status_maps_worker_unauthorized_to_workspace_worker_error
create_endpoint_uses_default_keep_alive_limits_when_missing
delete_endpoint_delegates_endpoint_and_template_cleanup
```

Use fake `RunpodApi`, fake `SecretStore`, and fake `ApiKeyIdentityProvider` following the existing fake patterns in `secrets_storage/service.rs` and `remote_workspace/service.rs`.

- [ ] **Step 6: Run tests and commit**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml runpod
git add src-tauri/src/remote_workspace/providers/runpod
git commit -m "feat(runpod): implement provider traits"
```

Expected: PASS.

---

### Task 8: Register Provider And Final Verification

**Files:**
- Modify: `src-tauri/src/remote_workspace/registry.rs`
- Modify: `src-tauri/src/remote_workspace/providers/runpod/mod.rs`
- Modify: `src-tauri/README.md`

- [ ] **Step 1: Add registration constructor path**

Add a constructor or factory for production provider registration. Because application-level secret service wiring is outside this scope, keep the registry API capable of receiving an already-built provider:

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

- [ ] **Step 3: Run generated-client verification**

Run:

```bash
bun run generate:runpod
cargo test --manifest-path src-tauri/Cargo.toml --no-run
```

Expected: generation completes and native tests compile.

- [ ] **Step 4: Commit**

Run:

```bash
git add src-tauri/src/remote_workspace src-tauri/README.md src-tauri/src/generated package.json src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat(runpod): register remote workspace provider"
```

---

## Self-Review

- Spec coverage: generation, `.runpod.graphql` discovery, provider module layout, secret-service injection, HMAC derivation, HF flag propagation, placement max volume, RunPod REST lifecycle, provisioner worker status/error mapping, default keep-alive limits, and verification are covered by tasks.
- Placeholder scan: the plan contains no deferred sections, no deferred error handling, and no live RunPod tests in the default path.
- Type consistency: `SecretKey::ProvisionerTokenSecret`, `hmac_sha256_hex`, `StartProvisionerParams.requires_hugging_face_api_key`, `RemoteWorkspaceError::ProvisionerWorker`, and provider module paths are introduced before later tasks use them.
