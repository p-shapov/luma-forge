## 1. Domain Cleanup

- [x] 1.1 Remove broad `#[allow(dead_code)]` and `#![allow(dead_code)]` workarounds from native Rust source.
- [x] 1.2 Remove unused speculative domain types that are not needed by current live behavior.
- [x] 1.3 Resolve `domain::workflow::WorkflowCatalog` by either wiring it into live catalog behavior or deleting it.

## 2. Workspace Domain Model

- [x] 2.1 Add or complete a provider-agnostic `domain::workspace::Workspace` aggregate that owns Workspace lifecycle state and Provider Resource snapshot state.
- [x] 2.2 Add a Draft Workspace constructor that validates required identity/name inputs and initializes lifecycle state plus empty Provider Resource snapshots.
- [x] 2.3 Keep the domain Workspace independent from application services, command DTOs, SQLite repositories, Tauri runtime APIs, provider clients, and generated frontend binding traits.

## 3. Contract Mapping

- [x] 3.1 Add explicit mapping from domain Workspace to `workspace_contracts::Workspace` while preserving the existing serialized fields.
- [x] 3.2 Add any required mapping from application contract inputs into domain constructor inputs without introducing command DTO dependencies.
- [x] 3.3 Add focused tests for domain-to-contract mapping, including Draft lifecycle state and empty Provider Resource snapshots.

## 4. Workspace Setup Integration

- [x] 4.1 Update Workspace Setup creation to call the domain Draft Workspace constructor instead of hand-constructing `workspace_contracts::Workspace`.
- [x] 4.2 Preserve current placement-plan validation, provider key prerequisite checks, duplicate handling, SQLite row consistency behavior, and command error semantics.
- [x] 4.3 Update Workspace Setup tests so they prove created Workspace records are domain-authored and remain command/persistence compatible.

## 5. Verification

- [x] 5.1 Run `rg -n "allow\\(dead_code\\)|dead_code" src-tauri src` and confirm any remaining allowances are targeted and documented.
- [x] 5.2 Run `cargo test`.
- [x] 5.3 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 5.4 Run `cargo fmt`.
