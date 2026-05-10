## 1. Domain Model Consolidation

- [x] 1.1 Add `Serialize` and `Deserialize` derives to domain models used by bundled catalogs, provider inventory, provider setup snapshots, placement plans, profiles, and workspaces.
- [x] 1.2 Replace generic domain provisioning and endpoint profile structs with provider-discriminated domain profile unions.
- [x] 1.3 Replace generic domain Placement Plan usage with a provider-discriminated domain Placement Plan union.
- [x] 1.4 Move LumaForge RunPod profile configuration structs into the domain-owned profile model while keeping RunPod HTTP and GraphQL DTOs provider-local.
- [x] 1.5 Add or update domain tests for Draft Workspace construction and provider-discriminated placement/profile compatibility.

## 2. Catalog and Persistence Boundaries

- [x] 2.1 Update bundled catalog parsing to deserialize workflow catalogs, provisioning profiles, and endpoint profiles directly into domain types.
- [x] 2.2 Update bundled catalog validation to validate domain-native workflow, profile, and provider-specific config data.
- [x] 2.3 Update the Workspace Catalog repository trait to return and accept domain-native Workspace Catalog and Workspace records.
- [x] 2.4 Update SQLite workspace catalog persistence to serialize and deserialize domain Workspace JSON directly.
- [x] 2.5 Preserve SQLite indexed row consistency validation against the domain Workspace payload.

## 3. Service Layer Simplification

- [x] 3.1 Refactor Workspace Setup catalog reader and provider inventory gateway traits to use domain-native types.
- [x] 3.2 Refactor Workspace Setup service methods to accept domain values or service inputs composed of domain values.
- [x] 3.3 Refactor Workspace Setup placement validation to operate on provider-discriminated domain Placement Plans.
- [x] 3.4 Refactor Provider Setup service methods to accept domain provider ids and return domain setup results directly.
- [x] 3.5 Remove service-facing request/response DTOs that no longer provide a boundary distinct from commands or domain.

## 4. Command Boundary Mapping

- [x] 4.1 Update Workspace Setup command DTO mappers to map generated DTOs directly to and from domain types.
- [x] 4.2 Update Provider Setup command DTO mappers to map generated DTOs directly to and from domain types.
- [x] 4.3 Ensure command DTOs remain the only DTOs deriving `specta::Type`.
- [x] 4.4 Regenerate or verify generated TypeScript command bindings if the existing workflow requires it.
- [x] 4.5 Verify command names, serialized payload fields, and UI-safe error codes remain compatible.

## 5. Cleanup and Verification

- [x] 5.1 Delete `src-tauri/src/workspace/workspace_contracts.rs` after all references are removed.
- [x] 5.2 Delete obsolete setup/workspace service contract modules or reduce them to service input types only if still justified.
- [x] 5.3 Update native tests that asserted mappings through removed service DTO layers.
- [x] 5.4 Run `cargo test`.
- [x] 5.5 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 5.6 Run `cargo fmt`.

## 6. Domain Validator Split

- [x] 6.1 Create domain-owned validator modules grouped by validated concept, starting with profiles, placement, and provider inventory where rules exist.
- [x] 6.2 Keep bundled parsers/readers in `bundled` and make them delegate domain invariant checks to the new domain validators.
- [x] 6.3 Update Workspace Setup validation call sites to use domain placement/profile validators instead of bundled or service-local reusable rules.
- [x] 6.4 Add or update focused native tests for the moved validators and boundary error mapping.
- [x] 6.5 Run `cargo test`.
- [x] 6.6 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 6.7 Run `cargo fmt`.

## 7. Remaining Domain Validators

- [x] 7.1 Add domain-owned validators for remaining provider setup and workspace aggregate shapes.
- [x] 7.2 Use workspace validation at the SQLite workspace catalog boundary.
- [x] 7.3 Use provider setup validation after provider identity derivation.
- [x] 7.4 Add focused native tests for the remaining validators and boundary mappings.
- [x] 7.5 Run `cargo test`.
- [x] 7.6 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 7.7 Run `cargo fmt`.

## 8. Domain Module Layout

- [x] 8.1 Move domain model, validator, and local test files into concept directories.
- [x] 8.2 Preserve public domain model import paths and update validator call sites consistently.
- [x] 8.3 Run `cargo test`.
- [x] 8.4 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 8.5 Run `cargo fmt`.
