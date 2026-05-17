## Why

Worker ports are currently treated as native build configuration even though they are fixed worker/provider deployment contract values. Keeping them in `.env` and Cargo build output makes builds fail on non-secret configuration that should not vary per developer or release, and it leaves the RunPod endpoint value semantically misleading because it represents the internal ComfyUI HTTP port rather than an Endpoint Worker API port.

## What Changes

- Remove Provisioner Worker and RunPod Endpoint Worker port values from native build-time configuration.
- Stop requiring `LUMA_FORGE_PROVISIONER_WORKER_PORT` and `LUMA_FORGE_RUNPOD_ENDPOINT_WORKER_PORT` in the project `.env` or real build environment.
- Remove obsolete root dotenv examples for worker image refs because worker image selection is owned by the bundled Runtime Catalog and Workspace snapshots.
- Move fixed worker/provider port decisions into the native provisioning/provider implementation boundary.
- Rename or model the RunPod endpoint-side port according to its actual meaning when it remains needed: the internal ComfyUI HTTP port exposed by the endpoint container, not a generic Endpoint Worker port.
- Preserve Runtime Catalog ownership of worker image refs and Workspace snapshot pinning of selected runtime implementations.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `native-build-configuration`: Native builds no longer parse or emit worker port build configuration.
- `workspace-provisioning`: Workspace Provisioning uses provider/worker implementation constants for fixed RunPod deployment ports instead of build-provided configuration.

## Impact

- Affects root `.env` and `.env.example` worker configuration entries.
- Affects `src-tauri/build.rs`, `src-tauri/build/app_config`, and `src-tauri/src/app_config`.
- Affects `NativeAppState` construction of `WorkspaceProvisioningConfig`.
- Affects workspace provisioning and RunPod provider gateway contracts where worker ports are passed.
- Requires Rust tests and clippy/fmt for native changes.
