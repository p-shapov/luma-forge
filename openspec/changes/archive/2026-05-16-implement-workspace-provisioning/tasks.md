## 1. Domain and Persistence Foundations

- [x] 1.1 Add provider placement capability domain types, including endpoint keep-alive supported/unsupported variants.
- [x] 1.2 Extend `PlacementPlan::Runpod` with provider-specific `endpoint_keep_alive_seconds`.
- [x] 1.3 Validate RunPod endpoint keep-alive as `5..=3600` and default placement options to `5`.
- [x] 1.4 Add Workspace domain types for Workspace Provisioning Progress, provider-specific provisioning snapshots, and RunPod endpoint template snapshots.
- [x] 1.5 Update Workspace validation rules for provisioning, ready, failed, and draft states, including provider-specific RunPod template metadata invariants.
- [x] 1.6 Add Workspace Catalog migration support for the updated Workspace JSON shape and persistence version.
- [x] 1.7 Extend Workspace Catalog repository with find-by-id and transactional update operations that preserve indexed row consistency.
- [x] 1.8 Add repository tests for lifecycle transitions, snapshot persistence, corrupt row rejection, future-version migration rejection, and persisted RunPod keep-alive values.

## 2. Secret Storage

- [x] 2.1 Extend `src-tauri/src/secrets` with per-workspace Provisioner Worker bearer token write, read, and delete operations.
- [x] 2.2 Store provisioning tokens under a keyring scope/account separate from GPU Cloud Provider API Keys.
- [x] 2.3 Add secret-store tests proving provisioning token operations do not read, overwrite, or delete Provider API Key entries.
- [x] 2.4 Map secret-storage-owned provisioning token failures into Workspace Provisioning use-case errors.

## 3. RunPod Provider Operations

- [x] 3.1 Extend `provider::error` with provider-local provisioning error categories needed by RunPod REST operations.
- [x] 3.2 Add RunPod REST DTOs for network volume create/get/delete responses and status mapping.
- [x] 3.3 Add RunPod REST DTOs for pod create/get/delete responses, port/status mapping, and provisioning pod URL derivation.
- [x] 3.4 Add RunPod REST DTOs for serverless template create/get/delete responses and template status mapping.
- [x] 3.5 Add RunPod REST DTOs for endpoint create/get/delete responses, invoke URL derivation, and endpoint status mapping.
- [x] 3.6 Extend `RunPodClient` with network volume, pod, template, and endpoint REST methods while keeping existing GraphQL identity/inventory behavior intact.
- [x] 3.7 Add RunPod mapper tests for successful responses, unauthorized responses, unavailable responses, invalid payloads, and not-found observations.

## 4. Provider Registry Boundary

- [x] 4.1 Define `ProviderProvisioningGateway` in the Workspace Provisioning application boundary.
- [x] 4.2 Implement `ProviderProvisioningGateway` for `ProviderClientRegistry`.
- [x] 4.3 Ensure the registry reads Provider API Keys through the secret store for provisioning calls and never exposes them to service responses.
- [x] 4.4 Extend Workspace Setup provider inventory gateway output so `get_provider_placement_options` can return inventory plus placement capabilities.
- [x] 4.5 Add registry tests for RunPod dispatch, provider setup prerequisite failures, provider-local error mapping into Workspace Provisioning errors, and RunPod placement capabilities.

## 5. Provisioner Worker Client

- [x] 5.1 Create `src-tauri/src/provisioner_worker` with request, status, and error contracts for `POST /start`, `GET /status`, and `POST /cancel`.
- [x] 5.2 Implement the Provisioner Worker HTTP client with bearer authorization, JSON body bounds compatible with worker expectations, and UI-safe error mapping.
- [x] 5.3 Map worker phases and terminal statuses into Workspace Provisioning Progress inputs.
- [x] 5.4 Add Provisioner Worker client tests for authorized success, unauthorized failure, conflict, invalid payload, terminal failure, and unreachable worker cases.

## 6. Workspace Provisioning Service

- [x] 6.1 Create `src-tauri/src/workspace_provisioning` with service contracts, error types, planner, progress derivation, and per-workspace coordinator.
- [x] 6.2 Implement initiate validation and the durable `draft` to `provisioning` lifecycle transition.
- [x] 6.3 Implement sync planning so each sync performs at most one safe action derived from persisted Workspace metadata.
- [x] 6.4 Implement network volume creation, observation, snapshot persistence, and indeterminate-create handling.
- [x] 6.5 Implement provisioning pod creation, observation, bearer token storage, and active pod snapshot persistence.
- [x] 6.6 Implement Provisioner Worker start/status/cancel interaction and environment-prepared timestamp persistence.
- [x] 6.7 Implement provisioning pod termination, last pod snapshot persistence, active pod clearing, and token deletion.
- [x] 6.8 Implement RunPod serverless template creation/observation and persist `template_id` before endpoint creation.
- [x] 6.9 Implement RunPod serverless endpoint creation/observation with `workersMin = 0`, `workersMax = 1`, `scalerType = "REQUEST_COUNT"`, `scalerValue = 1`, and `idleTimeout` from persisted RunPod Placement Plan keep-alive seconds.
- [x] 6.10 Implement readiness validation that marks Workspace `ready` only when volume, template, endpoint, prepared environment, and no-active-pod invariants hold.
- [x] 6.11 Create `src-tauri/src/workspace_resource_cleanup` and implement shared known-resource cleanup behavior for endpoint, RunPod template, active provisioning pod, persistent volume, active worker cancellation, and provisioner token deletion.
- [x] 6.12 Implement provisioning cancellation as a return-to-draft policy over shared known-resource cleanup.
- [x] 6.13 Add service tests for resume after each persisted checkpoint, duplicate sync, failure preservation, shared cleanup ordering, cancellation success, and cancellation cleanup failure.

## 7. Command Boundary and Wiring

- [x] 7.1 Rename `get_provider_inventory` command to `get_provider_placement_options`.
- [x] 7.2 Update Workspace Setup command response DTOs to return `provider_inventory` and `placement_capabilities`.
- [x] 7.3 Add command-owned generated binding metadata for provider placement capabilities and updated RunPod Placement Plan keep-alive data.
- [x] 7.4 Add `commands/workspace_provisioning` request/response DTOs and remote generated binding metadata for Workspace Provisioning Progress.
- [x] 7.5 Add Tauri commands for initiate, sync, and cancel Workspace Provisioning.
- [x] 7.6 Extend command error mapping with UI-safe Workspace Provisioning error codes, endpoint keep-alive range validation errors, and retryability.
- [x] 7.7 Register renamed Workspace Setup command and new provisioning commands in the Tauri Specta command builder.
- [x] 7.8 Wire Workspace Provisioning service dependencies in `NativeAppState`, including provider registry, workspace catalog, secrets, provisioner worker client, coordinator, and worker build config values.
- [x] 7.9 Add command tests proving successful command responses, error mapping, placement capability output, and secret-safe generated payloads.

## 8. Verification

- [x] 8.1 Run `cargo test` for native changes.
- [x] 8.2 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 8.3 Run `cargo fmt`.
- [x] 8.4 Regenerate TypeScript command bindings if command contracts changed and verify generated files are current.
- [x] 8.5 Run `openspec status --change implement-workspace-provisioning` and confirm the change is apply-ready.
