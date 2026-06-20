# Native Backend

This directory contains the active Tauri native backend. Business workflows live in application services, Tauri commands stay as adapters, and domain models stay independent from Tauri runtime APIs, persistence adapters, UI concerns, and provider SDK details.

## Domain Entities

`Workspace` is the persisted user runtime instance. It has a stable identity, references the selected workflow version, records the current lifecycle state, and stores runtime-specific data needed to manage the configured execution environment.

`WorkspaceState` is the native lifecycle status for a workspace: not provisioned, provisioning, ready, cleaning up, or invalid. Application services update it as lifecycle operations progress, and Tauri commands expose it to the UI as the authoritative workspace status.

`WorkspaceRuntime` is the runtime-specific part of a workspace. It keeps runtime configuration and implementation details out of shared workspace rules. The current runtime variant is `Runpod`.

`WorkflowCatalog` is the bundled catalog of curated workflow presets available to the app. A `WorkflowPreset` is a named workflow option, and each `WorkflowRevision` describes one version: runtime preset, execution schema and input bindings, required model assets, required storage size, credential requirements, and runtime contract requirements.

`RuntimeCatalog` is the bundled catalog of worker/runtime contracts. A `RuntimeContract` groups available revisions for a runtime contract, while `RuntimeContractReference` pins the exact contract id and version a workflow needs during workspace lifecycle operations.

`LifecycleOperation` is the durable record for a background workspace operation such as provision or cleanup. It belongs to a workspace, tracks operation state and timestamps, and may include runtime-specific step payload so progress can resume, report status, and diagnose failures.

`RunpodRuntime` is the RunPod-specific workspace runtime data. It combines the chosen `RunpodPlacementPlan` with `RunpodResources`, the RunPod resource identifiers created during provisioning: network volume, provisioner pod, endpoint, and template. The native backend uses those identifiers to find the existing RunPod resources for later progress updates, cleanup and delete.

`RunpodPlacementOptions` are RunPod-discovered choices available before workspace creation: datacenters, GPU types, VRAM, and maximum supported volume size. A `RunpodPlacementPlan` is the selected datacenter, GPU type, and volume size persisted into the workspace.

## Workspace Service and Runtimes

`WorkspaceService` is the application service that manages the workspace lifecycle. It creates workspaces, starts provision/cleanup/delete tasks in the background, records lifecycle journal state and emits workspace events.

`WorkspaceRuntime` is the service-facing trait for runtime-specific lifecycle operations: provision, cleanup, and delete. The dispatcher inside `WorkspaceService` selects the implementation from the workspace runtime data while the service keeps the shared lifecycle rules and persistence flow.

At the moment, `provider/runpod` is the concrete implementation. `RunpodWorkspaceRuntime` implements `WorkspaceRuntime` for `Runpod(RunpodRuntime)` workspaces, keeps resources provision and cleanup steps, RunPod API calls, provisioner coordination, and runtime catalog access behind the provider boundary.

When adding a new workspace runtime, extend `WorkspaceService` only where the shared lifecycle contract needs a new runtime entry point or dispatch case. Keep provider-specific orchestration behind a `WorkspaceRuntime` implementation, and use `WorkspaceRuntimeContext` to persist workspace and lifecycle journal progress.

## Native Support Files

During pre-production development, local SQLite schema bootstrap or compatibility checks may reject stale state from an earlier build. Stop the app before deleting the local database file.

Native support files are configured centrally in `src/app/support.rs` and live under the Tauri `app_data_dir()`. On macOS, the path pattern is:

```text
~/Library/Application Support/com.luma-forge/
```

Current support files:

- `native.sqlite`: native SQLite database for workspace catalog and lifecycle journal state.
- `logs/`: native diagnostics logs, including `luma-forge.log.YYYY-MM-DD`.

Deleting `native.sqlite` removes local native state only. It does not clean up remote provider resources such as RunPod volumes, pods, endpoints, or templates. Manual deletion is developer troubleshooting guidance for pre-production state; it is not a supported production migration or downgrade path.

### Using Logs

Native diagnostics are structured `tracing` logs. UI-facing errors stay safe and compact; native logs keep the trace ID, operation context, redacted error message, source chain, and span close timing needed to debug failures.

- For command failures, copy `traceId` from `NativeCommandError` and search the log file for that exact value.
- For lifecycle operation failures, copy `traceId` from `LifecycleOperationChangedEvent` and search the log file for that exact value.
- Use the matching log entry to identify the command or lifecycle operation, native error code, operation ID, workspace ID, redacted error message, and source chain.
- Trace backward from the logged boundary: command adapter to application service to provider/storage call, or lifecycle runner to lifecycle step to provider/worker call.

## Verification

For native backend changes, run from the repository root:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

If Tauri command contracts or exported Specta types change, also run:

```bash
bun run codegen:commands
bun run build
bun run lint
```
