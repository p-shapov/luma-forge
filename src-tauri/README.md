# Native Backend

This directory contains the active Tauri native backend. Business workflows live in application services, Tauri commands stay as adapters, and domain models stay independent from Tauri runtime APIs, persistence adapters, UI concerns, and provider SDK details.

## Workspace Runtimes

A `Workspace` owns common workspace identity and workflow selection. `WorkspaceRuntime` describes how that workspace is operated at runtime.

At the moment, only the RunPod runtime is available: `Runpod(RunpodRuntime)`. It represents RunPod-backed GPU infrastructure and RunPod resources. Future runtimes should be added only when they have a clear owner service and operation boundary.

When adding a new workspace runtime, keep runtime-specific orchestration behind its own service boundary and persist long-running work through the lifecycle journal.

## RunPod Runtime

`provisioned_remote` currently contains the RunPod runtime implementation for workspace setup, lifecycle operation creation, background lifecycle execution, deletion, and RunPod API integration. `RunpodRuntimeService` owns the service-level workflow surface and receives one concrete `RunpodRuntimeClient`.

RunPod API adapters must return only UI-safe errors and snapshots. Do not return raw provider responses, request bodies, API keys, bearer tokens, worker tokens, Hugging Face keys, credential-bearing URLs, SDK debug output, or environment dumps.

### Runtime Client Boundary

The RunPod client boundary should expose RunPod resource primitives only:

- placement options
- network volume create/delete
- provisioner pod start/status/terminate
- serverless template create/delete
- serverless endpoint create/delete

It should not duplicate the full lifecycle workflow. Orchestration belongs in `RunpodRuntimeService` and the lifecycle runner.

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
