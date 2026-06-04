# Provision Workspace State Machine Design

## Context

The active native backend has a `remote_workspace` skeleton in `src-tauri/src/remote_workspace/`. Its `provision_workspace` implementation currently creates a remote volume on the first call and starts a provisioner on the second call, but the rest of the provisioning lifecycle is not implemented.

The intended behavior is the legacy `workspace_provisioning::sync` model: each call performs at most one bounded provider or worker action, updates workspace state, and returns. A caller repeats the operation until the workspace reaches a terminal state.

This design updates the existing skeleton in place. It does not introduce a new orchestration abstraction.

## Goals

- Implement the missing `provision_workspace` state machine in the active `src-tauri` backend.
- Preserve legacy sync mechanics: one bounded action per call.
- Remove provider existing-resource preflight behavior.
- Remove `observe_workspace`.
- Remove provider `observe_*` resource methods and params.
- Remove existing-resource error variants.
- Add provisioner-worker error variants matching the legacy worker failure cases.
- Keep resource snapshots UI-safe and preserve known snapshots after failures for later cleanup.

## Non-Goals

- No concrete RunPod provider implementation.
- No new persistence layer.
- No frontend changes.
- No compatibility shim for old observe/conflict behavior.
- No retry scheduler or internal loop that blocks until provisioning completes.

## State Machine

`RemoteWorkspaceService::provision_workspace` remains the public orchestration entry point. It inspects `RemoteProvisioningStatus`, performs the next valid action, updates the cloned workspace, and returns.

The normal path is:

1. `NotStarted`: create a remote volume, store `remote_volume`, set phase to `StartingRemoteProvisioner`, and return.
2. `StartingRemoteProvisioner`: require `remote_volume`, start the provisioner, store `remote_provisioner`, set phase to `RunningRemoteProvisioner { status: Pending }`, and return.
3. `RunningRemoteProvisioner`: require `remote_provisioner`, call `get_provisioner_status`, store the returned status in the phase, and return for non-terminal statuses.
4. `RunningRemoteProvisioner` with `Succeeded`: move to `CleaningUpRemoteProvisioner { terminal_status: Succeeded }` and return.
5. `RunningRemoteProvisioner` with `Failed { code, message }`: move to `CleaningUpRemoteProvisioner { terminal_status: Failed { code, message } }` and return.
6. `CleaningUpRemoteProvisioner`: terminate the stored provisioner and clear `remote_provisioner` only after successful termination.
7. If the terminal worker status was success, move to `CreatingRemoteEndpoint` and return.
8. If the terminal worker status was failure, set `Failed { phase: Some(RunningRemoteProvisioner { status }), code, message }`, preserve known resource snapshots, and return.
9. `CreatingRemoteEndpoint`: require `remote_volume`, create the endpoint, store `remote_endpoint`, set `Completed`, set progress to `100`, and return.
10. `Completed`: return the workspace unchanged and do not call the provider.
11. `Failed`: return `InvalidWorkspaceState` and do not call the provider.

The service does not internally loop across these steps. If the worker reports `Pending`, `Starting`, or `Running`, the call only updates the phase status and returns. If it reports a terminal status, the next call performs cleanup.

The active `RemoteProvisioningPhase::CleaningUpRemoteProvisioner` variant must carry the terminal worker status. Without that payload, the next sync call cannot distinguish success cleanup from failure cleanup without polling the worker again or performing multiple actions in one call.

## Resource Observation Removal

Existing-resource discovery is removed entirely from the active provider boundary.

Delete:

- `RemoteWorkspaceService::observe_workspace`
- `ObserveVolumeParams`
- `ObserveProvisionerParams`
- `ObserveEndpointParams`
- `RemoteVolumeProvider::observe_volume`
- `RemoteProvisionerProvider::observe_provisioner`
- `RemoteEndpointProvider::observe_endpoint`
- `RemoteWorkspaceError::ExistingVolume`
- `RemoteWorkspaceError::ExistingProvisioner`
- `RemoteWorkspaceError::ExistingEndpoint`

Provider adapters should create only the resources requested by the current provisioning phase. They should not search for same-named resources as a conflict preflight.

## Error Handling

Provider request errors remain command-level errors before provisioning has enough state to record a meaningful workspace failure. Existing-resource conflicts no longer have dedicated handling.

`RemoteWorkspaceError` must be updated with active provisioner-worker cases matching the legacy `WorkspaceProvisioningFailureCode` worker variants:

- `ProvisionerWorkerTokenMissing`
- `ProvisionerWorkerTokenInvalid`
- `ProvisionerWorkerUnauthorized`
- `ProvisionerWorkerUnavailable`
- `ProvisionerWorkerConflict`
- `ProvisionerWorkerResponseInvalid`
- `ProvisionerWorkerFailed`
- `ProvisionerWorkerAssetDownloadFailed`
- `ProvisionerWorkerAssetAuthRequired`
- `ProvisionerWorkerPathValidationFailed`
- `ProvisionerWorkerStepTimeout`
- `ProvisionerWorkerUnexpectedError`

These replace generic worker failure strings where the worker can report or the native layer can derive a specific cause. They must remain UI-safe and must not include raw worker payloads, bearer tokens, provider API keys, filesystem dumps, or console logs.

Invalid workspace states fail before provider calls. Examples:

- `StartingRemoteProvisioner` without a stored `remote_volume`.
- `RunningRemoteProvisioner` without a stored `remote_provisioner`.
- `CreatingRemoteEndpoint` without a stored `remote_volume`.
- `Failed` provisioning status passed back into `provision_workspace`.

Provisioner termination failure is a terminal workspace failure, not an endless cleanup loop. If cleanup follows worker success, the workspace is marked failed with a UI-safe cleanup failure code/message and the provisioner snapshot is preserved. If cleanup follows worker failure, the workspace is marked failed with the original worker failure code/message and the provisioner snapshot is preserved. In both cases, later delete or retry logic can clean up known resources.

When worker status is `Failed { code, message }`, the service attempts to terminate the provisioner before marking the workspace `Failed`. A provider error from that termination attempt does not prevent the workspace from becoming `Failed`.

## Progress

Progress remains coarse and UI-safe:

- Volume created: `25`
- Provisioner started: `50`
- Worker running or terminal status observed: `50` to `75`
- Provisioner cleanup complete: `75`
- Endpoint created and completed: `100`

Exact percentages are implementation details, but they must be monotonic on the happy path.

## Testing

Update existing `remote_workspace` Rust unit tests in place.

Remove tests for:

- `observe_workspace` conflict detection.
- provider observe call ordering.
- preflight observe calls from `NotStarted`.

Add or update tests for:

- `NotStarted` creates only a volume.
- `StartingRemoteProvisioner` starts only a provisioner.
- `RunningRemoteProvisioner` polls status and stores non-terminal status.
- terminal success status moves to cleanup instead of creating an endpoint immediately.
- terminal failure status moves to cleanup before marking failed.
- cleanup after success terminates the provisioner and moves to endpoint creation.
- cleanup after worker failure terminates the provisioner and marks failed.
- cleanup after success termination failure marks the workspace failed and preserves `remote_provisioner`.
- cleanup after worker failure termination failure still marks the workspace failed with the original worker failure and preserves `remote_provisioner`.
- endpoint creation marks the workspace completed.
- invalid phase/resource combinations fail without provider calls.
- completed workspaces return unchanged.
- failed workspaces reject continuation.

Add focused worker error mapping tests for:

- missing provisioner token maps to `ProvisionerWorkerTokenMissing`.
- invalid provisioner token maps to `ProvisionerWorkerTokenInvalid`.
- worker unauthorized maps to `ProvisionerWorkerUnauthorized`.
- worker unavailable or unreachable maps to `ProvisionerWorkerUnavailable`.
- worker conflict maps to `ProvisionerWorkerConflict`.
- malformed worker response maps to `ProvisionerWorkerResponseInvalid`.
- generic worker failure maps to `ProvisionerWorkerFailed`.
- asset download failure maps to `ProvisionerWorkerAssetDownloadFailed`.
- asset auth failure maps to `ProvisionerWorkerAssetAuthRequired`.
- path validation failure maps to `ProvisionerWorkerPathValidationFailed`.
- step timeout maps to `ProvisionerWorkerStepTimeout`.
- unexpected worker error maps to `ProvisionerWorkerUnexpectedError`.

Registry and fake provider tests must compile without any observe methods.

## Verification

Run from the repository root:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

If the adjusted provisioning phase shape is exported through generated command bindings during implementation, also run:

```bash
bun run codegen:commands
bun run build
bun run lint
```
