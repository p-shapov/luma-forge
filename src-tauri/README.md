# Native Backend

This directory contains the active Tauri native backend. Business workflows live in application services, Tauri commands stay as adapters, and domain models stay independent from Tauri runtime APIs, persistence adapters, UI concerns, and provider SDK details.

## Workspace Runtimes

A `Workspace` owns common workspace identity and workflow selection. `WorkspaceRuntime` describes how that workspace is operated at runtime.

At the moment, only the RunPod runtime is available: `Runpod(RunpodRuntime)`. It represents RunPod-backed GPU infrastructure and RunPod resources. Future runtimes should be added only when they have a clear owner service and operation boundary.

When adding a new workspace runtime, keep runtime-specific orchestration behind its own service boundary and persist long-running work through the lifecycle journal.

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
