## 1. Reset First Implementation Attempt

- [x] 1.1 Revert service-local input/output wrapper structs introduced for Provider Setup and Workspace Setup.
- [x] 1.2 Restore Provider Setup and Workspace Setup services to use application/service contract types directly.
- [x] 1.3 Keep service tests focused on application/service contracts rather than command-owned DTOs.

## 2. Command DTO Ownership

- [x] 2.1 Add command-owned Provider Setup DTOs that derive `specta::Type` and preserve existing generated payload shapes.
- [x] 2.2 Add command-owned Workspace Setup DTOs that derive `specta::Type` and preserve existing generated payload shapes.
- [x] 2.3 Move command handlers to accept and return command-owned DTOs.
- [x] 2.4 Add explicit mappings between command-owned DTOs and application/service contracts.

## 3. Remove Specta From Application Contracts

- [x] 3.1 Remove `specta::Type` derives and imports from Provider Setup application/service contracts.
- [x] 3.2 Remove `specta::Type` derives and imports from Workspace Setup application/service contracts.
- [x] 3.3 Remove `specta::Type` derives and imports from nested Workspace/Profile/Catalog application contracts where command DTOs now own generated bindings.
- [x] 3.4 Keep existing `serde` derives in application/service contracts where they already exist.

## 4. Compatibility Tests

- [x] 4.1 Verify generated command names, request fields, response fields, and UI-safe error behavior remain compatible.
- [x] 4.2 Add focused command DTO mapping tests where compatibility is not covered by existing tests.
- [x] 4.3 Regenerate/check generated TypeScript command bindings and confirm payload compatibility.

## 5. Verification

- [x] 5.1 Run `cargo test`.
- [x] 5.2 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 5.3 Run `cargo fmt`.
- [x] 5.4 Run `bun run build`.
- [x] 5.5 Run `bun run lint --fix`.
