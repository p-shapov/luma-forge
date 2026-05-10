## Context

The native layer already separates command DTOs from application/service contracts. Workspace Setup currently uses those application contracts directly for Draft Workspace creation, persistence, and command response mapping. That is acceptable for serializable data exchange, but it also means `workspace_contracts::Workspace` is currently the only live lifecycle-bearing model.

At the same time, `src-tauri/src/domain/workspace.rs` contains Workspace lifecycle and Provider Resource snapshot vocabulary behind a module-level `#![allow(dead_code)]`. `src-tauri/src/domain/workflow.rs` also contains an item-level `#[allow(dead_code)]` for `WorkflowCatalog`. Broad allowances hide whether domain code is authoritative, speculative, or obsolete. Workspace Provisioning will soon require lifecycle transitions where invalid combinations must not be representable by casual struct construction in services.

## Goals / Non-Goals

**Goals:**

- Remove broad native `dead_code` allowances.
- Keep spec-defined Workspace lifecycle and Provider Resource status vocabulary in the domain, with narrow comments where upcoming provisioning behavior will construct currently unused variants.
- Make unused domain code either part of live behavior or remove it.
- Introduce a real Workspace domain aggregate that authors Draft Workspace creation and can later own provisioning transitions.
- Keep dependency direction clean: application services depend on domain; domain does not depend on services, command DTOs, persistence, Tauri runtime APIs, provider clients, or generated frontend binding concerns.
- Preserve generated command payload compatibility and the existing persisted Workspace JSON shape.

**Non-Goals:**

- Do not implement Workspace Provisioning.
- Do not add Provider Resource creation, lifecycle transition commands, cancellation, or cleanup flows.
- Do not redesign the SQLite schema.
- Do not split every existing application contract from every persistence DTO in this change.
- Do not introduce new dependencies.

## Decisions

### Domain code must not use broad dead-code suppressions

Native source code will remove broad `#[allow(dead_code)]` and `#![allow(dead_code)]` workarounds. In domain modules, unused code must usually be treated as either unfinished design or obsolete design. A narrow `#[allow(dead_code)]` is permitted only for spec-defined domain vocabulary, and it must have an adjacent comment explaining which upcoming flow will construct it.

Rationale: the domain is where invariants should become explicit. Suppressing unused domain code makes it unclear whether a type is authoritative, obsolete, or waiting for future implementation.

Alternative considered: delete unused lifecycle/status variants until provisioning lands. Rejected because those states are already part of the Workspace Provisioning flow language and should remain in the domain.

### Workspace lifecycle is authored by a domain aggregate

Add `domain::workspace::Workspace` as the authoritative lifecycle model for Workspace state construction. The first live operation should be Draft creation. The Workspace Setup service validates orchestration prerequisites, then calls a domain constructor such as `Workspace::new_draft(...)`.

Rationale: Workspace Provisioning needs lifecycle transitions that combine enum state, Provider Resource snapshots, retained cleanup metadata, and environment preparation state. Putting the first constructor in domain now prevents the provisioning implementation from scattering transition rules across services and persistence code.

Alternative considered: keep `workspace_contracts::Workspace` as the lifecycle model and add helper functions around it. Rejected because the contract must remain shaped for serialization and boundary mapping, while the domain should be free to encode invariants without command or persistence constraints.

### Contracts remain serializable boundary shapes

Keep `workspace_contracts::Workspace` as the application/persistence-facing serializable shape for this change. Add explicit mapping between the domain Workspace and the contract Workspace. The SQLite repository may continue storing the existing serialized contract shape.

Rationale: the current persistence contract is already covered by command compatibility and row consistency tests. Keeping that shape avoids a migration while still creating a proper domain authority.

Alternative considered: split application contracts, persistence DTOs, and domain models all at once. Rejected as too broad for a pre-provisioning cleanup and likely to obscure the core lifecycle fix.

### Remove unused speculative domain types

If a domain type is not used by live behavior after this change and is not spec-defined near-term vocabulary, remove it. For example, `domain::workflow::WorkflowCatalog` should either be used by catalog-level validation/mapping or deleted.

Rationale: unused domain types create false architecture. Keeping only used domain concepts makes dependency direction and ownership easier to audit.

Alternative considered: keep every unused type as future placeholder code. Rejected because only spec-defined near-term domain vocabulary should receive a targeted allowance.

## Risks / Trade-offs

- Mapping duplication between domain and contracts -> Keep mappings explicit and local to workspace/domain contract boundaries; defer abstraction until repeated transition mappings become painful.
- Existing tests construct `workspace_contracts::Workspace` directly -> Update only tests that exercise Workspace Setup behavior to assert domain-authored output, while keeping command contract tests focused on payload compatibility.
- Domain constructor may initially look thin -> Accept this as a foundation; even Draft creation has real invariants around lifecycle state and empty Provider Resource snapshots.
- Removing speculative types could require re-adding them later -> Re-add only when a live behavior needs them, with tests that exercise the behavior.

## Migration Plan

1. Remove broad `dead_code` allowances from native source code.
2. Delete unused speculative domain types that are not needed by this change and are not spec-defined near-term domain vocabulary.
3. Add or complete the Workspace domain aggregate for Draft construction.
4. Add explicit mapping between domain Workspace and the existing Workspace application contract.
5. Update Workspace Setup service to create Draft Workspace records through the domain aggregate before persistence.
6. Preserve generated command payloads and persisted JSON shape.
7. Run native verification: `cargo test`, `cargo clippy --fix --allow-dirty --allow-staged`, and `cargo fmt`.

Rollback is a source-level revert. The change has no durable data migration.

## Open Questions

- Should future provisioning transition methods live entirely on the Workspace aggregate, or should a small domain service coordinate multi-resource transition decisions? Default: start with aggregate methods for local state invariants and introduce a domain service only if provisioning transition logic outgrows the aggregate.
