# RunPod Provider Implementation Design

## Goal

Implement the RunPod remote workspace provider for `src-tauri/src/remote_workspace/provider.rs`.

The provider must create and delete RunPod network volumes, CPU provisioner pods, serverless endpoint templates, and serverless endpoints. It must fetch placement options from RunPod GraphQL, call the provisioner worker HTTP service for status, and keep all secrets behind trusted native-layer paths.

## Architecture

The RunPod provider lives under:

```text
src-tauri/src/remote_workspace/providers/runpod/
```

`RunpodRemoteWorkspaceProvider` implements the existing remote workspace traits:

- `RemotePlacementOptionsProvider`
- `RemoteVolumeProvider`
- `RemoteProvisionerProvider`
- `RemoteEndpointProvider`
- `RemoteWorkspaceProvider`

Provider API details stay inside the RunPod adapter. Public provider trait methods continue to use parameter structs from `src-tauri/src/remote_workspace/provider.rs` and return LumaForge domain snapshots and errors. The implementation updates those provider parameter structs only where the provider boundary needs additional domain-safe input, such as passing `workflow_preset.requires_hugging_face_api_key` into `StartProvisionerParams`.

The provider module is split by responsibility:

- `api.rs`: wraps handwritten typed RunPod REST and GraphQL HTTP calls behind provider-sized methods.
- `provisioner_worker.rs`: calls the provisioner HTTP service and maps worker statuses and worker errors.
- `mapping.rs`: converts RunPod and worker responses into LumaForge domain snapshots and UI-safe errors.
- `config.rs`: owns RunPod provider constants, including default keep-alive limits and provider-owned request defaults.
- `mod.rs`: exposes `RunpodRemoteWorkspaceProvider` and trait implementations.

The provider does not receive raw Hugging Face tokens or RunPod secrets directly. It receives already-initialized `SecretsStorageService` instances for Hugging Face and RunPod, and uses the secrets-storage abstraction for provisioner worker token derivation. Initializing and wiring those services happens outside the provider and is not part of this implementation scope. Raw secret values only flow into outbound RunPod calls, provisioner calls, container environment creation, or secrets-module-local HMAC derivation. They must not appear in snapshots, generated frontend types, persisted workspace JSON, test fixtures, logs, or returned error messages.

`SecretsStorageService` is constructed for a single `SecretKey` and gains a method such as:

```rust
async fn hmac_sha256_hex(&self, message: &str) -> Result<String, SecretsStorageError>
```

The method retrieves the service-scoped secret internally, computes HMAC-SHA256 over the provided message, and returns a lowercase hex digest. Callers never receive the raw secret.

## API Client Strategy

RunPod code generation is removed from scope.

GraphQL introspection is not available for RunPod production usage. RunPod documents `https://api.runpod.io/graphql` as the API endpoint, but schema introspection is disabled as a production security policy. The public `https://graphql-spec.runpod.io/` page is documentation, not an introspection endpoint. This matches the standard recommendation to disable production GraphQL introspection and means generated GraphQL bindings are not reliable for this project. References: `https://www.apollographql.com/blog/why-you-should-disable-graphql-introspection-in-production` and `https://www.answeroverflow.com/m/1379125332036157470`.

REST/OpenAPI generation is also removed from scope. The RunPod REST OpenAPI document can still be used as reference material, but OpenAPI Generator adds local Java or container execution requirements and creates a large generated surface for a small provider boundary.

The provider uses handwritten typed request and response structs with `serde` and `reqwest`. These structs cover only the REST endpoints and GraphQL placement query required by the current provider contract. Type safety comes from local request builders, response structs, serialization tests, and mapping tests rather than generated API clients.

No `generate:runpod` command, generated RunPod modules, `.runpod.graphql` files, OpenAPI Generator configuration, or provider-specific generated code is added.

`src-tauri/README.md` is updated only to correct the provider path to `src/remote_workspace/providers/<provider_name>/`.

## Resource Lifecycle

The existing `RemoteWorkspaceService` keeps orchestration ownership. The RunPod provider implements only resource primitives.

### Placement Options

`get_provider_placement_options` uses RunPod GraphQL to fetch listed datacenters with storage support plus GPU availability.

It maps RunPod fields into:

- `RemoteDatacenterPlacementOption.id`
- `RemoteDatacenterPlacementOption.name`
- `RemoteGpuPlacementOption.id`
- `RemoteGpuPlacementOption.name`
- `RemoteGpuPlacementOption.vram_bytes`
- `RemoteGpuPlacementOption.availability_score`

`RemotePlacementOptions.max_persistent_storage_volume_size_bytes` is set in this method from RunPod's documented network volume maximum: `4000 GB`, converted to bytes.

### Volume Creation

`create_volume` calls:

```text
POST /networkvolumes
```

The request includes:

- `dataCenterId` from `CreateVolumeParams.datacenter_id`
- a deterministic LumaForge workspace-scoped name
- `size` converted from bytes to GB

The response id becomes `RemoteVolumeSnapshot.id`.

### Provisioner Pod Creation

`start_provisioner` calls:

```text
POST /pods
```

The request creates a CPU compute pod in the selected datacenter with:

- the provisioner image ref
- the existing network volume id
- `volumeMountPath = /workspace`
- the provisioner worker HTTP port exposed through RunPod
- provider-owned CPU/container disk defaults

The environment includes:

- `LUMA_FORGE_PROVISIONER_BEARER_TOKEN`
- `LUMA_FORGE_WORKSPACE_MOUNT_PATH`
- `LUMA_FORGE_HUGGING_FACE_API_KEY` when the selected `workflow_preset.requires_hugging_face_api_key` flag is `true`

`RemoteWorkspaceService` passes `workspace.workflow_preset.requires_hugging_face_api_key` into `StartProvisionerParams`; the provider uses that flag to decide whether to retrieve and inject the Hugging Face API key.

The provisioner bearer token is derived by calling the secret service with the workspace id:

```text
runpod_secrets.hmac_sha256_hex(workspace_id)
```

The token is encoded as lowercase hex so it is ASCII-only and satisfies the provisioner worker minimum bearer token length.

The returned `RemoteProvisionerSnapshot` stores the RunPod pod id and a status URL built from RunPod's public HTTP port mapping.

### Provisioner Status

`get_provisioner_status` calls:

```text
GET <status_url>/status
Authorization: Bearer <derived-token>
```

Worker status mapping:

- `idle` maps to `RemoteProvisionerStatus::Pending`
- `running` maps to `RemoteProvisionerStatus::Running`
- `succeeded` maps to `RemoteProvisionerStatus::Succeeded`
- `failed` maps to `RemoteProvisionerStatus::Failed { code, message }`

Worker API error responses map into `RemoteProvisioningError` variants and are recorded by the service as `RemoteProvisioningStatus::Failed`. Worker-specific failures must not be flattened into generic invalid provisioning state.

### Provisioner Pod Deletion

`terminate_provisioner` calls:

```text
DELETE /pods/{podId}
```

RunPod 404 maps to `RemoteWorkspaceError::RemoteProvisionerNotFound`.

### Endpoint Creation

`create_endpoint` first calls:

```text
POST /templates
```

The template request includes the endpoint image ref, mount path, environment, and serverless template settings.

After template creation, it calls:

```text
POST /endpoints
```

The endpoint request includes:

- created `templateId`
- selected datacenter
- selected GPU type
- network volume id
- worker min/max defaults
- scaler defaults
- keep-alive configuration

If `CreateEndpointParams.keep_alive_limits` is `None`, RunPod provider constants provide default, min, and max values before converting to RunPod endpoint fields such as `idleTimeout`.

The endpoint id and invocation URL become `RemoteEndpointSnapshot`.

### Endpoint Deletion

`delete_endpoint` performs the required multi-step cleanup:

1. `GET /endpoints/{endpointId}`
2. read `template.id` or `templateId`
3. `DELETE /endpoints/{endpointId}`
4. `DELETE /templates/{templateId}`

RunPod 404 for the endpoint maps to `RemoteWorkspaceError::RemoteEndpointNotFound`.

If endpoint deletion succeeds but template deletion fails, the provider returns a UI-safe provider error so the template leak is visible.

### Volume Deletion

`delete_volume` calls:

```text
DELETE /networkvolumes/{networkVolumeId}
```

RunPod 404 maps to `RemoteWorkspaceError::RemoteVolumeNotFound`.

## Error Handling

RunPod API errors map to UI-safe errors:

- `401`: `ProviderApiError::Unauthorized`
- `403`: `ProviderApiError::InsufficientPermissions`
- `429`: `ProviderApiError::RateLimited`
- timeout: `ProviderApiError::Timeout`
- operation-specific `404`: `RemoteVolumeNotFound`, `RemoteProvisionerNotFound`, or `RemoteEndpointNotFound`
- other failures: `ProviderApiError::RequestFailed { message }` with bounded, credential-free text

Returned errors must not include raw RunPod responses, auth headers, API keys, Hugging Face keys, derived provisioner tokens, env maps, request bodies, stack traces, command output, or credential-bearing URLs.

Provisioner worker API failures map to existing `RemoteProvisioningError` variants where possible:

- worker `401`: `ProvisionerWorkerUnauthorized`
- worker connection or timeout failure: `ProvisionerWorkerUnavailable` or `ProvisionerWorkerStepTimeout`
- worker `409`: `ProvisionerWorkerConflict`
- invalid JSON or unsupported shape: `ProvisionerWorkerResponseInvalid`
- failed worker status with `asset_download_failed`: `ProvisionerWorkerAssetDownloadFailed`
- failed worker status with auth-related asset error: `ProvisionerWorkerAssetAuthRequired`
- failed worker status with path validation error: `ProvisionerWorkerPathValidationFailed`
- other failed worker statuses: `ProvisionerWorkerUnexpectedError` or `ProvisionerWorkerFailed`

The service records worker failures in `RemoteProvisioningStatus::Failed`.

## Testing

Default tests do not make live RunPod calls.

Provider unit tests cover:

- placement option mapping and max volume size conversion
- network volume request construction and response mapping
- CPU provisioner pod request fields
- provisioner token derivation without exposing the token in returned data
- Hugging Face env injection and omission
- provisioner worker status mapping
- provisioner worker error mapping into `RemoteProvisioningError`
- endpoint template-before-endpoint creation order
- endpoint default keep-alive handling
- endpoint deletion order, including template cleanup failure
- provider not-found mapping for volume, provisioner, and endpoint deletion
- RunPod API error mapping

Service-level tests cover worker-specific errors becoming `RemoteProvisioningStatus::Failed { error: RemoteProvisioningError::* }`.

Typed API verification covers:

- request serialization for each REST operation
- GraphQL placement query payload shape
- response deserialization for REST and GraphQL responses used by the provider
- compile verification under `cargo test --manifest-path src-tauri/Cargo.toml`

Live RunPod integration tests may be added later behind explicit environment flags, but they are not part of the default verification path.
