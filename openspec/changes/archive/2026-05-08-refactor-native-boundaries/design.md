## Context

The native layer already has two implemented flows: GPU Cloud Provider Setup and Workspace Setup. The code is mostly separated by concern, but a few boundaries still point in the wrong direction:

- Provider implementations return use-case-specific errors from setup and workspace setup.
- Command-safe error DTOs live in provider setup code even though workspace commands also depend on them.
- Workspace persistence is already isolated in `workspace/`, but one persisted provider identifier is still hardcoded.
- `provider_setup.rs` is much larger than neighboring modules and mixes command contracts, errors, service orchestration, gateway traits, and tests.

Workspace Provisioning will add provider mutations, durable lifecycle transitions, resource snapshots, cleanup metadata, concurrency handling, and progress reporting. If these boundaries are not cleaned up first, provisioning is likely to duplicate provider error mapping and persistence concerns across flows.

## Goals / Non-Goals

**Goals:**

- Keep Tauri command handlers as thin adapters that map use-case results into command-safe responses.
- Keep provider implementations independent from application use-case errors.
- Keep provider-specific transport, GraphQL shapes, API quirks, and mapping code inside provider modules.
- Keep workspace durable contracts and repository code under the `workspace/` module.
- Preserve current command behavior and generated frontend contract shape unless a type path update is mechanically required.
- Make the native module layout easier to extend for Workspace Provisioning.

**Non-Goals:**

- Do not implement Workspace Provisioning.
- Do not introduce another GPU cloud provider.
- Do not change React UI behavior.
- Do not change the current generated command request/response payload semantics.
- Do not change provider setup or workspace setup product behavior.

## Decisions

### Move command errors to the command boundary

`NativeCommandError` and `NativeCommandErrorCode` will move from provider setup code into `commands`, or a command-adjacent module owned by the Tauri boundary.

Rationale: command errors are part of the generated frontend contract. Provider setup and workspace setup can expose use-case errors, but they should not own the shared command DTO.

Alternative considered: leave `NativeCommandError` in provider setup and continue importing it from workspace setup. That keeps the smallest diff but makes provider setup an accidental owner of cross-flow command infrastructure.

### Use provider-local errors inside provider clients

Provider clients will return a provider-local error type, for example `ProviderClientError`, instead of returning `ProviderSetupError` or `WorkspaceSetupError` directly.

Rationale: RunPod transport and response parsing failures are provider concerns. Application services decide whether a provider error maps to `invalid_provider_api_key`, `provider_api_unavailable`, `provider_identity_unavailable`, or future provisioning errors.

Alternative considered: keep separate provider methods returning each flow's error type. That creates convenient local code but makes provider modules depend on every use case they support.

### Keep gateway traits owned by use cases

Application services will continue to define the gateway traits they need, such as identity validation for provider setup and inventory lookup for workspace setup. `ProviderRegistry` will implement those traits and perform provider-error-to-use-case-error mapping.

Rationale: use cases define what they need from infrastructure; infrastructure adapts itself to that need. This keeps application services testable without making provider clients generic over all future flows.

Alternative considered: define one large provider trait in `provider`. That centralizes provider operations but tends to become a broad service locator as provisioning and cleanup are added.

### Preserve the `workspace/` directory with explicit `workspace_` file names

Workspace setup, catalog, contracts, and tests will remain under `src-tauri/src/workspace/` with explicit file names such as `workspace_setup.rs` and `workspace_catalog.rs`.

Rationale: the directory groups the aggregate, while the prefixed file names remain clear in editor tabs, test output, and search results.

Alternative considered: use short file names like `setup.rs` and `catalog.rs`. That is idiomatic inside a module but was less explicit for this codebase.

### Split provider setup only along responsibility boundaries

Provider setup should be split into a directory only if the split separates real responsibilities: contracts, service, errors, and tests. The refactor should not create small files that merely hide a single cohesive unit.

Rationale: `provider_setup.rs` is now large enough to obscure ownership, but the split should be driven by dependencies and testing, not line count alone.

Alternative considered: leave provider setup as a single file until provisioning. That avoids churn but keeps command-error ownership and tests mixed into the setup service.

### Derive persisted provider identifiers from domain values

Workspace catalog persistence will derive the stored provider identifier from `Workspace.gpu_cloud_provider_id` instead of hardcoding `runpod`.

Rationale: the schema already stores a provider id; persistence should reflect the workspace record, even while v1 supports only RunPod.

Alternative considered: leave the hardcoded value because v1 has only one provider. That works today but weakens persistence correctness and testability.

## Risks / Trade-offs

- Provider-local errors may initially look like extra indirection -> Mitigation: keep the enum small and map it close to gateway trait implementations.
- Moving command errors can touch generated bindings -> Mitigation: regenerate bindings through existing tests and verify frontend build/lint if generated TypeScript changes.
- Splitting provider setup may produce noisy diffs -> Mitigation: perform file moves first, then semantic edits, and keep command behavior unchanged.
- Future provisioning may need more repository methods than this refactor introduces -> Mitigation: only prepare boundaries now; design provisioning-specific persistence transitions in the provisioning change.

## Migration Plan

1. Add command-owned error DTOs and update command result aliases.
2. Move use-case error mappings to the command boundary or command-adjacent mapping code.
3. Introduce provider-local error types and convert RunPod identity/inventory methods to return them.
4. Map provider-local errors in `ProviderRegistry` implementations of setup and workspace gateway traits.
5. Fix workspace catalog provider id persistence to derive from the workspace value.
6. Split provider setup files if the command-error move leaves clear contract/service/test boundaries.
7. Run native verification and regenerate/check command bindings.

Rollback is a source-level revert of the refactor because no durable schema migration or runtime data migration is introduced.

## Open Questions

- Should `NativeCommandError` live in `commands/command_error.rs` or a root `native_error.rs` module? Default: keep it under `commands` because it is a frontend command contract.
- Should provider setup be split in this change or deferred until after command-error extraction? Default: split if it reduces imports and test ownership in the same pass.
