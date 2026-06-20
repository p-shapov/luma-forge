# Native Backend

This directory contains the active Tauri native backend. Business workflows live in application services, Tauri commands stay as adapters, and domain models stay independent from Tauri runtime APIs, persistence adapters, UI concerns, and provider SDK details.

## Workspace Runtimes

A `Workspace` owns common workspace identity and workflow selection. `WorkspaceRuntime` describes how that workspace is operated at runtime.

At the moment, only the RunPod runtime is available: `Runpod(RunpodRuntime)`. It represents RunPod-backed GPU infrastructure and RunPod resources. Future runtimes should be added only when they have a clear owner service and operation boundary.

When adding a new workspace runtime, keep runtime-specific orchestration behind its own service boundary and persist long-running work through the lifecycle journal.

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
