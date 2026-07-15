# Native Backend

This directory contains the Tauri native backend. Application services own business workflows, Tauri commands stay at the inbound facade, and application models remain independent from Tauri, persistence, UI, and provider transport details.

## Architecture

- `facade/` defines Tauri commands and events, Specta-generated UI contracts, request mapping, and UI-safe error mapping.
- `application/` owns workspace, runtime, and secret models, services, events, and ports.
- `adapters/` implements application ports with the bundled catalog, SQLite, the OS keyring, and provider integrations.
- `infra/` owns low-level bundled catalog, SQLite, and keyring mechanisms.
- `providers/` owns RunPod and Hugging Face HTTP and GraphQL transport.
- `lib.rs` is the composition root and fails startup if diagnostics, persistence, bundled resources, providers, or interrupted-operation recovery cannot initialize.

## Core Models

`Workspace` is a persisted identity with an exact workflow `CatalogRef` and an optional `Runtime`. The absence of a runtime means the workspace is not provisioned; there is no separate workspace lifecycle state.

`Runtime` combines provider-neutral `RuntimeState` with a tagged `RuntimeProvider`. Shared states are `Provisioning`, `Ready`, `CleaningUp`, and `Failed`. The current provider is RunPod; its placement configuration and remote resource identifiers stay in native application state, while facade DTOs omit resource identifiers.

`RuntimeOperation` is the durable journal entry for a background provision or cleanup. It stores the runtime kind, operation kind and state, provider-specific progress, timestamps, and an optional diagnostics trace ID. Operation history remains after successful cleanup removes the runtime.

`WorkflowSummary` and `WorkflowDefinition` are application projections of the revisioned bundled catalog. A workflow revision resolves metadata, model assets, execution data, a runtime preset, and exact provisioner and endpoint runtime contracts.

## Application Flow

`WorkspaceService` validates workflow references and owns workspace creation, listing, and deletion. `RuntimeService` is the provider-neutral entry point for provision, cleanup, operation queries, and interrupted-operation recovery. It uses closed enum dispatch to `RunpodRuntimeService`, which owns RunPod-specific lifecycle orchestration; there is no runtime registry, factory, or facade-owned lifecycle routing.

Runtime transitions persist the workspace runtime and operation atomically through `RuntimeTransitionRepository`. SQLite keeps provider state and operation progress as typed tagged JSON payloads on the provider-neutral `workspace_runtimes` and `runtime_operations` rows. Events are emitted only after the transaction commits.

The facade exposes UI-safe commands and the `workspace_changed`, `workspace_deleted`, and `runtime_operation` events. It maps transport DTOs and errors but does not access SQLite, bundled files, provider clients, or keyring storage directly.

RunPod and Hugging Face API keys are validated before being stored in the OS keyring. They are write-only from the UI perspective: commands may set, inspect identity for, or delete a credential, but never return the raw secret.

## Native Support Files

During pre-production development, local SQLite schema bootstrap or compatibility checks may reject stale state from an earlier build. Stop the app before deleting the local database file.

Native support files are configured centrally in `src/lib.rs` and live under the Tauri `app_data_dir()`. On macOS, the path pattern is:

```text
~/Library/Application Support/com.luma-forge/
```

Current support files:

- `db.sqlite`: native SQLite database for workspaces, attached runtime state, and runtime operation history.
- `diagnostics.log`: native diagnostics log.

Deleting `db.sqlite` removes local native state only. It does not clean up remote provider resources such as RunPod volumes, pods, endpoints, or templates. Manual deletion is developer troubleshooting guidance for pre-production state; it is not a supported production migration or downgrade path.

### Using Logs

Native diagnostics are JSON call logs produced by `#[diagnostic]`. Commands create root traces, nested operations share that trace, and detached runtime work preserves or restores it. Values are omitted by default and appear only when explicitly marked safe or redacted.

- For command failures, copy `traceId` from `CommandError` and search the log file for that exact value.
- For background runtime failures, use `RuntimeOperationEvent.operation.traceId` when present.
- Follow matching `call.start`, `call.success`, and `call.error` records by their `function`, trace ID, and span ID to find the failing command, service, adapter, or provider boundary.

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
