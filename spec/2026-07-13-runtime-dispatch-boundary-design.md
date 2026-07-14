# Runtime Dispatch Boundary Design

**Status:** Approved for implementation planning

> **Persistence update:** Provider-specific SQLite extension tables and persistence dispatch are superseded by [`2026-07-14-runtime-provider-payload-persistence-design.md`](./2026-07-14-runtime-provider-payload-persistence-design.md). The application lifecycle dispatch decisions in this document remain current.

## Goal

Move runtime lifecycle dispatch out of the Tauri facade and into the application layer while keeping provider-specific persistence dispatch inside the composite SQLite adapter.

## Current Problems

`facade::RuntimeDispatcher` currently chooses `RunpodRuntimeService` for provision, cleanup, and interrupted-operation recovery. Cleanup also loads the workspace before choosing the service, and recovery groups operations by `RuntimeKind`. Those are application workflow responsibilities rather than transport mapping.

The dispatcher returns `RunpodRuntimeError`, even though its error categories describe provider-neutral runtime lifecycle failures. This makes the facade contract depend on the only runtime implementation.

`RunpodRuntimeService` also narrows shared runtime unions directly and reads the first workflow requirement as if every requirement were a RunPod requirement. Adding another runtime variant would therefore force lifecycle code changes in the RunPod service.

The SQLite runtime persistence dispatcher is different: it participates in one transaction that saves the provider-neutral runtime anchor and operation together with provider-specific state and progress. That dispatch belongs to the persistence adapter and is not application workflow routing.

## Decision

Runtime dispatch has two distinct owners:

1. The application layer owns lifecycle dispatch. A provider-neutral `RuntimeService` selects the provider-specific application service for provision, cleanup, and interrupted-operation recovery.
2. The composite SQLite adapter owns persistence dispatch. It selects provider-specific state and progress mapping while preserving the existing transaction boundary.

The Tauri facade maps transport DTOs and errors only. Provider-specific adapters continue implementing one provider port each and do not dispatch between runtimes.

No trait registry, factory, plugin loader, or dynamic runtime registration is introduced.

## Target Flow

```text
Tauri command
    -> FacadeState: validate and map transport DTO
    -> application::runtimes::RuntimeService
         -> closed match by RuntimeKind or provision command
         -> RunpodRuntimeService
         -> future runtime service
    -> RuntimeTransitionContext
    -> RuntimeTransitionRepository
    -> SQLite transaction orchestration
         -> closed persistence match
         -> RunPod persistence module
         -> future runtime persistence module
```

## Application Runtime Service

`application::runtimes::RuntimeService` is the single provider-neutral lifecycle entry point exposed to the facade.

It owns:

- the closed application dispatch over supported runtime kinds;
- workspace loading before cleanup;
- loading and grouping interrupted operations before recovery;
- delegation to the matching provider-specific runtime service.

It exposes:

- `start_provision(command: ProvisionRuntime)`;
- `start_cleanup(workspace_id: &str)`;
- `recover_interrupted()`.

`ProvisionRuntime` is an application command enum. Its RunPod variant contains the existing `ProvisionRunpodRuntime` command. The facade maps `ProvisionRuntimeInput` into this enum but does not choose a service.

The service depends on the existing workspace and runtime-operation repository ports plus the concrete supported runtime services. The runtime set remains closed and compiler-checked.

Provider-specific queries that are not lifecycle dispatch, such as RunPod placement lookup, continue calling the corresponding application service directly from the facade. `FacadeState` therefore keeps a `RunpodRuntimeService` clone for placement queries and a `RuntimeService` for provider-neutral lifecycle commands; bootstrap constructs the RunPod service once and clones it into both consumers.

## Runtime Error Boundary

The lifecycle error contract becomes `application::runtimes::RuntimeError`.

The existing provider-neutral variants move from `RunpodRuntimeError` into the common runtime boundary without changing their semantics:

- `WorkspaceNotFound`;
- `WorkflowNotFound`;
- `AlreadyProvisioned`;
- `RuntimeFailed`;
- `OperationInProgress`;
- `NotProvisioned`;
- `CredentialMissing`;
- `InvalidCredential`;
- `ProviderUnavailable`;
- `CatalogUnavailable`;
- `PersistenceUnavailable`;
- `InvalidTransition`.

Shared persistence, operation, secret-store, and state-transition errors map into `RuntimeError` in the common runtime module. RunPod provider and catalog errors map into it inside the RunPod module.

The facade maps only `RuntimeError` to command-specific error codes. It no longer imports a RunPod lifecycle error type.

## Provider Narrowing

The closed shared enums own their narrowing operations:

- `RuntimeProvider::as_runpod()`;
- `RuntimeProvider::as_runpod_mut()`;
- `RuntimeContractRequirements::as_runpod()`.

`RunpodRuntimeService` uses these operations instead of matching shared unions inside lifecycle functions. A missing attached runtime remains `NotProvisioned`; an attached runtime delegated to the wrong provider is `InvalidTransition`.

RunPod workflow requirements are found across the complete requirements collection with `find_map`, rather than taking `.first()`. No RunPod requirements is `CatalogUnavailable`.

Adding a future runtime will still extend the central closed enums and their narrowing methods, but it will not require changing RunPod lifecycle orchestration.

## Facade Responsibilities

The facade continues to own:

- Tauri commands and Specta registration;
- request validation;
- DTO-to-application-command mapping;
- application-model-to-DTO mapping;
- application-error-to-command-error mapping;
- Tauri event projection.

It no longer owns:

- choosing a runtime service;
- loading a workspace to decide cleanup routing;
- grouping interrupted operations by runtime kind.

A `match` used only to translate a tagged transport DTO into `ProvisionRuntime` is mapping, not lifecycle dispatch, and remains in the facade.

## Adapter Responsibilities

Provider adapters remain provider-specific and implement one application port. `RunpodRuntimeProviderAdapter` never selects another provider.

The existing SQLite runtime persistence dispatcher remains because it owns provider-specific mapping within the generic atomic transition save. Generic workspace, operation, and transition repositories remain unchanged except for any import adjustments required by the error rename.

Bundled catalog and keyring matches remain data mapping by a closed discriminator; they are not lifecycle dispatch.

## Non-Goals

- Adding a second runtime.
- Dynamic runtime registration or external plugins.
- A common `RuntimeService` trait.
- Changes to SQLite tables or persisted values.
- Changes to Tauri command names, DTO shapes, generated TypeScript bindings, or frontend behavior.
- Refactoring provider-specific lifecycle implementations beyond the narrowing and error-boundary changes.

## Verification

The implementation must leave observable behavior unchanged and demonstrate:

- facade lifecycle methods call the application `RuntimeService` rather than a facade dispatcher;
- cleanup routing uses the attached runtime kind inside the application layer;
- interrupted operations are grouped and recovered inside the application layer;
- RunPod lifecycle methods return the common `RuntimeError`;
- RunPod model and requirement narrowing is centralized outside lifecycle workflow bodies;
- existing facade error codes remain unchanged;
- existing runtime transition and SQLite dispatch tests continue to pass.

Required repository checks:

```text
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```
