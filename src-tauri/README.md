# Native Backend

This directory contains the active Tauri native backend. Business workflows live in application services, Tauri commands stay as adapters, and domain models stay independent from Tauri runtime APIs, persistence adapters, UI concerns, and provider SDK details.

## Workspace Runtimes

A `Workspace` owns common workspace identity and workflow selection. `WorkspaceRuntime` describes how that workspace is operated at runtime.

At the moment, only the RunPod runtime is available: `Runpod(RunpodRuntime)`. It represents RunPod-backed GPU infrastructure and RunPod resources. Future runtimes should be added only when they have a clear owner service and operation boundary.

When adding a new workspace runtime, keep runtime-specific orchestration behind its own service boundary and persist long-running work through the lifecycle journal.

## Diagnostic Logs

Native diagnostics are structured `tracing` logs for command spans, command
failures, and lifecycle operation failures. UI-facing errors stay safe and
compact; native logs keep the diagnostic ID, operation context, redacted error
message, source chain, and span close timing needed to debug the failure.

Native diagnostics are saved to the Tauri app log directory. On macOS for this app identifier, the path is:

```text
~/Library/Logs/com.luma-forge/luma-forge.log.YYYY-MM-DD
```

### Using Logs

- For command failures, copy `diagnosticId` from `NativeCommandError` and search the log file for that exact value.
- For lifecycle operation failures, copy `diagnosticId` from `LifecycleOperationChangedEvent` and search the log file for that exact value. Normal lifecycle events use `diagnosticId: null`.
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
