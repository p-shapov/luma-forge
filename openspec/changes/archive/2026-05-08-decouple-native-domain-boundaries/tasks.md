## 1. Provider-Owned Profile Contracts

- [x] 1.1 Move RunPod provisioning profile config, endpoint profile config, and serverless scaling config types from bundled catalog contracts into the RunPod provider boundary.
- [x] 1.2 Update bundled catalog parsing and validation to use provider-owned RunPod config types without making bundled contracts the shared workspace API owner.
- [x] 1.3 Update workspace placement/profile contract imports so workspace code no longer depends on `bundled::bundled_catalog_contracts` for shared profile types.
- [x] 1.4 Add or update tests that prove bundled catalog reads and workspace placement validation still accept the existing RunPod bundled profiles.

## 2. Domain And Command DTO Separation

- [x] 2.1 Remove `serde` and `specta::Type` derives from pure domain models under `src-tauri/src/domain`.
- [x] 2.2 Introduce command-facing DTOs for domain data currently returned to React, preserving command behavior and UI-safe fields.
- [x] 2.3 Add explicit mapper functions between domain/application types and command DTOs at the command or use-case contract boundary.
- [x] 2.4 Keep Workspace Catalog persistence round-trips compatible by using explicit persistence/DTO records where serialized storage still needs `serde`.

## 3. Inventory Authorization Error Mapping

- [x] 3.1 Update RunPod inventory lookup to classify `401` and `403` responses as provider authorization failures.
- [x] 3.2 Add a workspace setup error path that maps provider authorization failure to `invalid_provider_api_key` and marks it non-retryable.
- [x] 3.3 Add tests for unauthorized RunPod inventory responses and command error mapping.

## 4. Verification

- [x] 4.1 Regenerate command bindings if command DTO ownership or exported types change.
- [x] 4.2 Run `cargo test`.
- [x] 4.3 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 4.4 Run `cargo fmt`.
- [x] 4.5 Run `bun run build` and `bun run lint --fix` if generated frontend bindings or frontend imports change.
