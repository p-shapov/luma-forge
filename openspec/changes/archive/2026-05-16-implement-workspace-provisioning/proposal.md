## Why

Workspace Setup can persist complete `Draft` Workspace records, and the Provisioner and Endpoint Worker contracts already exist, but the Native Layer cannot yet turn a saved workspace into a usable remote RunPod runtime. This change implements the native-owned Workspace Provisioning flow so a `Draft` Workspace can create provider resources, prepare its mounted ComfyUI environment, persist durable checkpoints, and become `Ready` without exposing provider secrets to React.

## What Changes

- Add a native Workspace Provisioning application service with initiate, sync, and cancel operations.
- Add a durable sync-driven provisioning state machine that performs at most one safe provider or worker action per sync iteration.
- Extend Workspace Catalog persistence with transactional workspace lookup and update operations for lifecycle and provider-resource snapshots.
- Extend Workspace domain metadata with provider-specific RunPod provisioning snapshot data, including a per-user RunPod serverless template identifier required for future cleanup.
- Extend the existing provider registry and RunPod client with provisioning resource operations for RunPod network volumes, temporary provisioning pods, serverless templates, and serverless endpoints.
- Add a native Provisioner Worker HTTP client for `POST /start`, `GET /status`, and `POST /cancel` with bearer-token authorization.
- Extend secure secret storage with per-workspace Provisioner Worker bearer tokens stored separately from Provider API Keys.
- Rename the Workspace Setup provider placement read command to `get_provider_placement_options` and return provider inventory plus placement capability metadata.
- Extend RunPod Placement Plan data with provider-specific endpoint keep-alive seconds used for RunPod endpoint `idleTimeout`.
- Add command-boundary contracts for Workspace Provisioning responses that return authoritative Workspace metadata plus derived Workspace Provisioning Progress.
- Keep Client / React UI implementation out of scope except generated command contract exposure.

## Capabilities

### New Capabilities

- `workspace-provisioning`: Native-owned provisioning of a saved `Draft` Workspace into `Ready` by creating RunPod resources, preparing the runtime volume, checkpointing progress, and preserving cleanup metadata on failure.

### Modified Capabilities

- `workspace-setup`: Rename provider placement options command, expose provider placement capabilities, and persist RunPod-specific endpoint keep-alive selection in the Placement Plan.
- `native-boundaries`: Extend provider registry and secret-store boundary requirements to cover provisioning gateways and per-workspace Provisioner Worker bearer tokens without coupling provider clients or secret infrastructure to provisioning command DTOs.

## Impact

- Affected native modules: `src-tauri/src/domain/workspace`, `src-tauri/src/workspace_catalog`, `src-tauri/src/provider/registry.rs`, `src-tauri/src/provider/runpod`, `src-tauri/src/secrets`, `src-tauri/src/app_state.rs`, `src-tauri/src/commands`.
- New native modules: `src-tauri/src/workspace_provisioning`, `src-tauri/src/provisioner_worker`, `src-tauri/src/commands/workspace_provisioning`.
- New reusable cleanup module: `src-tauri/src/workspace_resource_cleanup`, initially used by provisioning cancellation and later by the public Workspace Resource Cleanup command.
- Workspace Setup command contract impact: `get_provider_inventory` is replaced by `get_provider_placement_options`; generated bindings and command tests must be updated.
- Placement model impact: `PlacementPlan::Runpod` gains provider-specific `endpoint_keep_alive_seconds` with RunPod default `5` and valid range `5..=3600`.
- Provider API impact: adds RunPod REST calls for network volumes, pods, templates, and endpoints while retaining existing GraphQL identity/inventory calls.
- Persistence impact: Workspace JSON gains provider-specific provisioning metadata; Workspace Catalog update operations must preserve indexed row consistency.
- Security impact: Provider API Keys remain keyring-only; per-workspace Provisioner Worker bearer tokens are stored in a separate keyring scope and removed after the provisioning pod is confirmed no longer needed.
- Cleanup impact: full Workspace Resource Cleanup remains out of scope, but provisioning must persist RunPod `template_id` and other resource identifiers so future cleanup can delete every created provider resource.
