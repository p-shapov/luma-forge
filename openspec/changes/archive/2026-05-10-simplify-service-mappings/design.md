## Context

The native layer currently has three overlapping model shapes for much of Workspace Setup:

- domain models in `src-tauri/src/domain`
- workspace application contracts in `src-tauri/src/workspace/workspace_contracts.rs` and `workspace_setup_contracts.rs`
- command DTOs in `src-tauri/src/commands`

This creates repeated mappings around service logic. It also leaves some domain invariants, especially provider/profile/placement compatibility, represented by generic type parameters and runtime checks instead of by the domain shape itself.

The new direction is to make domain models the canonical native shape for local durable state and bundled catalog data. Command DTOs remain the generated frontend contract and the only place that derives `specta::Type`.

## Goals / Non-Goals

**Goals:**

- Remove service-layer DTO mappings where services can use domain types directly.
- Delete the redundant workspace application contract layer.
- Let domain types derive `Serialize` and `Deserialize` for native catalog parsing and SQLite JSON persistence.
- Keep `specta::Type` and frontend binding concerns out of domain modules.
- Represent provider-specific provisioning profiles, endpoint profiles, and placement plans as provider-discriminated domain unions.
- Preserve command payload compatibility and existing UI-safe error behavior.

**Non-Goals:**

- Do not implement Workspace Provisioning behavior.
- Do not change React command names, generated TypeScript payload semantics, or user-facing error codes.
- Do not add support for a second GPU cloud provider.
- Do not expose Provider API Keys or provider transport details through domain snapshots, command DTOs, logs, or diagnostics.

## Decisions

### Domain owns native serialization

Domain models will derive `Serialize` and `Deserialize` when they are the canonical shape for bundled catalogs, workspace snapshots, provider inventory, or setup snapshots. This removes the need for a parallel `workspace_contracts.rs` model graph.

Alternative considered: keep separate serializable application contracts and map them into domain in services. This preserves a pure domain model, but the current application is a local native app whose durable catalog/workspace shape is part of the native domain. The extra layer adds more duplication than protection.

### Command DTOs remain the Specta boundary

Command-facing request and response structs will continue to live under `src-tauri/src/commands` and derive `specta::Type`. Commands map between generated DTOs and domain types before calling services or before returning responses.

Alternative considered: derive `specta::Type` on domain types and return them directly. This would reduce mapping further, but it would couple domain model evolution to React binding generation and UI payload compatibility.

### Services are domain-native

Provider Setup and Workspace Setup services will accept domain values or small service input structs containing domain values. Services will return domain results directly. Service code should not depend on command DTOs or application DTO modules.

Alternative considered: keep request/response DTOs for every service method. This can be useful when a service boundary is remote or public, but here it duplicates the command boundary without adding a durable contract.

### Provider-specific placement is modeled structurally

Provisioning profiles, endpoint profiles, and placement plans will become provider-discriminated domain unions. For v1 these unions contain RunPod variants. A RunPod placement plan contains RunPod provisioning and endpoint profiles directly, so incompatible provider/profile combinations are harder to construct.

Alternative considered: keep generic domain profiles such as `ProvisioningProfile<C>` and `PlacementPlan<P, E>`. This is flexible but leaves the provider relationship implicit and requires extra runtime checks at service boundaries.

### Domain validation is split by validated concept

Domain invariants should be validated by small domain-owned validator modules grouped around the concept being validated, such as `profiles_validator`, `placement_validator`, `provider_inventory_validator`, and similar type/aggregate-focused validators where needed. Bundled parsers and readers remain in `bundled`, but they should delegate validation of deserialized domain values to these domain validators and translate validation failures into bundled-reader errors at the infrastructure boundary.

Alternative considered: keep a single bundled catalog validator that validates workflow, profile, and provider-specific data after parsing. That keeps parsing and validation close together, but it leaves business invariants in an infrastructure module and makes it harder to reuse the same validation from services or persistence boundaries.

### Provider HTTP DTOs stay provider-local

RunPod GraphQL request and response DTOs remain in the RunPod provider implementation. Domain may own LumaForge RunPod profile configuration because that configuration is part of bundled catalog/workspace data, but domain must not import provider HTTP response DTOs.

Alternative considered: keep all RunPod-named structs in `provider/runpod`. That keeps the domain provider-agnostic, but it also forces domain profile types to carry provider-owned config payloads and keeps workspace setup dependent on provider contract modules for catalog semantics.

## Risks / Trade-offs

- Native persistence shape becomes more directly tied to domain model shape -> Keep command DTO compatibility separate, and preserve SQLite row consistency validation when reading domain JSON.
- Domain becomes provider-aware for profile and placement concepts -> Keep the provider-aware surface limited to domain catalog/placement configuration, not provider transport shapes.
- Removing DTO layers may make a broad refactor harder to review -> Perform the implementation in small slices: domain serialization, catalog reader, repository, services, commands, tests.
- Existing archived specs conflict with the new direction -> Update `native-boundaries`, `workspace-setup`, and `gpu-cloud-provider-setup` deltas before implementation.

## Migration Plan

1. Add serde derives and provider-discriminated profile/placement types to the domain layer.
2. Move LumaForge RunPod profile configuration types out of provider HTTP contract ownership and into domain-owned catalog/profile modeling.
3. Split validation into domain-owned validators grouped by validated concept while keeping bundled parsers/readers in `bundled`.
4. Update bundled catalog parsing to deserialize domain types directly and delegate domain invariant checks to the relevant domain validators.
5. Update workspace catalog persistence to serialize and deserialize domain workspaces directly.
6. Update Workspace Setup services and repositories to use domain types directly.
7. Update Provider Setup services to accept domain inputs and return domain setup snapshots directly.
8. Update command DTO mappers so generated payloads remain compatible while mapping directly to/from domain types.
9. Remove obsolete workspace and setup application contract modules.
10. Run native verification: `cargo test`, `cargo clippy --fix --allow-dirty --allow-staged`, and `cargo fmt`.

Rollback strategy: because this is an internal native refactor with no intended command contract changes, rollback is a code revert before release. Persisted workspace JSON compatibility must be checked if any persisted shape changes after users have records.

## Open Questions

- Should existing persisted workspace JSON compatibility be preserved exactly, or is a local development reset acceptable before release?
- Should domain RunPod profile config types live in `domain::profiles` or in a provider-specific domain module such as `domain::provider_profiles`?
