## Context

The native layer has separate modules for command handlers, provider setup, workspace setup, provider clients, secret storage, bundled catalogs, and workspace persistence. The remaining boundary leak is that application/service contract types also carry generated frontend binding derives through `specta::Type`.

The first implementation direction introduced service-local input/output wrappers. That technically separated service methods from command DTOs, but it created near-duplicate types where the existing contracts were already good application contracts. A cleaner direction is to keep reusable application contracts in Provider Setup and Workspace Setup, leave `serde` where it is useful, and move only generated frontend binding ownership to command DTOs.

## Goals / Non-Goals

**Goals:**

- Keep Provider Setup and Workspace Setup application services using application/service contract types.
- Remove `specta::Type` from application/service contract types.
- Keep `serde` derives on existing contracts where they are already useful for parsing, persistence, or stable snapshot serialization.
- Add command-owned request/response DTOs that derive `specta::Type` for generated frontend bindings.
- Map command DTOs to application contracts before service calls and map service results back to command DTOs before returning to React.
- Preserve existing command names, serialized payload fields, response shapes, error codes, and user-visible behavior.

**Non-Goals:**

- Do not implement Workspace Provisioning.
- Do not add lifecycle transition repository methods.
- Do not remove existing `Workspace` lifecycle or Provider Resource snapshot fields.
- Do not redesign the workspace persistence schema.
- Do not split bundled catalog parsing and persistence serialization away from application contracts in this change.
- Do not remove `serde` from contracts where it is currently used.

## Decisions

### Treat setup contracts as application contracts

Existing Provider Setup and Workspace Setup contract modules will remain the reusable service-facing contract layer. Services may accept these types directly because they are no longer generated command DTOs once `specta::Type` ownership moves out.

Rationale: this avoids duplicating `Request` and `Input` structs with the same fields while preserving a clean generated-command boundary.

Alternative considered: keep service-local input/output wrappers. Rejected because the wrappers were visually noisy and added little value for setup flows whose existing contracts are already the application shape.

### Move Specta derives to command-owned DTOs

Command handlers will receive and return command-owned DTOs that derive `specta::Type`, `Serialize`, and `Deserialize` as needed. These DTOs should preserve the existing generated TypeScript shape.

Rationale: generated frontend bindings are a command boundary concern. Moving `specta::Type` to command DTOs prevents application contracts from being constrained by frontend binding generation.

Alternative considered: keep `specta::Type` on application contracts and rely on discipline not to treat them as command DTOs. Rejected because the derive itself makes the application contract part of the generated frontend surface.

### Keep serde in existing contracts for now

This change will not remove `serde` from Provider Setup, Workspace Setup, Workspace, profile, catalog, or provider-resource snapshot contracts. Some of these types are used for bundled catalog parsing and workspace persistence snapshots, so removing serde would require a broader infrastructure DTO split.

Rationale: the immediate boundary concern is Specta/generated binding ownership. Serde also supports non-command native infrastructure responsibilities today.

Alternative considered: remove all serialization derives from application contracts now. Rejected as too broad for this pre-provisioning cleanup.

### Map command DTOs explicitly

Mappings should stay close to command handlers or command-adjacent modules. Service tests should use application contracts. Command tests should cover mapper behavior where it protects compatibility.

Rationale: explicit mapping keeps the external command contract stable and makes conversion points visible before provisioning adds more commands.

Alternative considered: use generic conversion abstractions or macros. Rejected until repeated mapping code becomes demonstrably painful.

## Risks / Trade-offs

- More command DTO definitions -> Keep them command-local and mirror existing generated payloads intentionally.
- Nested command DTO expansion -> Move Specta ownership outward in coherent slices; avoid partial command DTOs that still depend on application types deriving `Type`.
- Generated binding drift -> Compare generated TypeScript before and after and run frontend build/lint.
- Serde remains in application contracts -> Accept for now; future changes can split persistence/catalog DTOs if needed.

## Migration Plan

1. Revert service-local input/output wrappers introduced by the first implementation attempt.
2. Keep Provider Setup and Workspace Setup services using application/service contracts.
3. Remove `specta::Type` derives from application/service contracts.
4. Add command-owned DTOs for generated Provider Setup and Workspace Setup command payloads.
5. Map command DTOs to application contracts and service outputs back to command DTOs.
6. Verify generated TypeScript payload compatibility.
7. Run native and frontend verification.

Rollback is a source-level revert. The change introduces no durable data migration and no external API migration.

## Open Questions

- Should command DTOs live in existing handler files or separate `*_command_contracts.rs` files? Default: separate files if DTO volume is more than a few small structs.
- Should nested Workspace/Profile/Catalog command DTOs be introduced all at once or only where generated binding derives currently force it? Default: move enough nested DTOs to remove `specta::Type` from application contracts without changing payloads.
