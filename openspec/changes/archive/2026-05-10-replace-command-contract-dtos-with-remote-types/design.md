## Context

Command contracts currently duplicate domain models for generated TypeScript bindings. Provider Setup duplicates the redacted setup snapshot, and Workspace Setup duplicates a much larger model graph for workflow catalogs, provisioning profiles, endpoint profiles, placement plans, provider inventory, and workspace snapshots. The duplicated command graphs exist primarily because Tauri Specta needs `specta::Type` implementations, while existing architecture keeps generated frontend binding derives out of domain modules.

The result is command contract code with mechanical `From` implementations between structurally identical types. The problem is most visible in Workspace Setup, but the architectural rule should apply to all command modules: command wrappers own the generated API boundary, while command modules may provide remote TypeScript binding metadata for domain models instead of maintaining duplicate runtime DTOs.

Specta supports remote type implementations through `#[specta(remote = ...)]`. A remote implementation lets a type in the command boundary provide `specta::Type` metadata for a domain type without adding `specta::Type` derives to the domain module itself.

## Goals / Non-Goals

**Goals:**

- Replace duplicated command model graphs with command-boundary remote `specta::Type` implementations for domain types.
- Preserve generated TypeScript command shapes where payload shapes are unchanged, and intentionally migrate Workspace Setup `PlacementPlan` payloads to the provider-discriminated domain shape.
- Keep domain modules free from `specta::Type` derives and Tauri command dependencies.
- Keep command-specific request and response wrappers in the command boundary.
- Remove mechanical mappings that only translate between identical command DTO and domain shapes.
- Keep secret handling and UI-safe command error behavior unchanged.

**Non-Goals:**

- Do not change command names.
- Do not change command names, UI-safe error codes, or secret-handling behavior.
- Do not simplify `CreateWorkspaceRequest` to id-only placement selection in this change.
- Do not introduce a second GPU cloud provider.
- Do not change bundled catalog, SQLite workspace, provider setup, or provider API semantics.
- Do not implement Workspace Provisioning behavior.

## Decisions

### Command boundary provides remote Type implementations

Command contract modules will define private or command-local remote Specta mirror structs/enums annotated with `#[specta(remote = domain_path::TypeName)]`. These remote definitions will implement `specta::Type` for the existing domain types used in command request and response wrappers.

This keeps frontend binding ownership in `src-tauri/src/commands` while avoiding a runtime DTO graph. The remote definitions are metadata for TypeScript export, not values passed through command handlers.

Alternative considered: derive `specta::Type` directly on domain models. That removes more command-boundary code, but it couples domain model definitions to generated frontend binding concerns and violates the current native-boundaries direction.

### Command wrappers remain explicit

Command-specific wrappers such as `GetGpuCloudProviderSetupResponse`, `SetupGpuCloudProviderRequest`, `GetWorkflowCatalogResponse`, `GetProviderInventoryRequest`, `GetWorkspaceCatalogResponse`, `CreateWorkspaceRequest`, and `CreateWorkspaceResponse` will remain command DTOs. They express command API shape and can continue to derive `Serialize`, `Deserialize`, and `Type`.

Nested payload fields may use domain types directly once those domain types have command-boundary remote `Type` implementations. For example, a response wrapper may hold a `domain_workflow::WorkflowCatalog` rather than a duplicated command `WorkflowCatalog`.

Workspace Setup command payloads will now expose the domain `PlacementPlan` directly, including its provider discriminator. This intentionally changes `CreateWorkspaceRequest.placement_plan` and returned `Workspace.placement_plan` from an untagged object to a provider-discriminated object with `gpu_cloud_provider_id: "runpod"`.

Alternative considered: return domain values directly from Tauri commands. That would erase useful command response structure and make future command compatibility harder to control.

### Verify serialized contract migration through generated diffs

Because this change removes explicit command DTO mapping and intentionally changes the Workspace Setup placement payload shape, verification should keep the binding export test running and use the committed `src/generated/commands.ts` diff as the generated contract review surface. Serialization tests should cover representative tagged payload behavior where native code owns command/domain conversion.

Alternative considered: add targeted string assertions over generated TypeScript. That catches drift during `cargo test`, but it duplicates the generated contract in brittle test strings and makes intentional contract migrations noisier.

### Keep request simplification out of scope

`CreateWorkspaceRequest` will still accept a full `PlacementPlan` containing selected catalog objects. A future change may move this to id-based selection resolved natively, but this proposal intentionally avoids that API redesign. The goal here is to remove duplicate DTO code across commands by exposing the domain provider-discriminated placement shape directly.

Alternative considered: combine remote type export with id-only placement input. That would improve authority boundaries, but it would also change the Workspace Setup command contract and expand the change beyond a focused refactor.

## Risks / Trade-offs

- Remote Specta definitions drift from domain fields -> Keep the binding export test, review committed `src/generated/commands.ts` diffs, and keep remote definitions grouped near command wrappers so drift is visible during compile or binding export.
- Workspace Setup frontend callers must add the nested placement provider discriminator -> Generated TypeScript makes the new required field explicit; update frontend call sites when they are introduced.
- Domain types accidentally gain command dependencies -> Keep all `specta::Type` implementations in command modules through `#[specta(remote = ...)]`; do not add `specta` imports to domain modules.
- Some command contract files remain large because remote definitions still mirror fields -> Size should still drop by removing runtime DTOs and mapping impls; if readability remains poor, split remote definitions by concept after compatibility is proven.
- Compile errors from orphan or duplicate remote implementations -> Introduce remote exports incrementally and remove command DTO types in small slices.

## Migration Plan

1. Add command-boundary remote `specta::Type` implementations for shared provider id and Provider Setup domain snapshot types.
2. Add command-boundary remote `specta::Type` implementations for Workspace Setup domain types.
3. Update command response wrappers to hold domain-native nested payloads.
4. Update `CreateWorkspaceRequest` to accept the domain provider-discriminated `PlacementPlan`.
5. Remove no-longer-needed command DTO structs/enums and mechanical `From` implementations from command contract modules.
6. Keep command-specific wrapper conversions where they still express command behavior.
7. Regenerate TypeScript bindings and compare generated command payload shapes against the previous generated contract.
8. Run native and frontend verification required by project instructions.

Rollback strategy: revert the refactor before release if generated command shapes drift or remote Specta support proves incompatible with the required tagged enum shapes.

## Open Questions

- Should remote type definitions live in existing `contracts.rs` files initially, or should the implementation split larger command contracts into concept-specific submodules during the same change?
- Should generated TypeScript compatibility and migration behavior eventually use snapshot tests, or is committed `src/generated/commands.ts` review sufficient?
