## 1. Native State Composition

- [x] 1.1 Add a native app state module that owns production infrastructure values and service construction helpers.
- [x] 1.2 Move provider setup coordinator ownership into the managed native app state.
- [x] 1.3 Add shared Workspace Catalog access through native app state, including app data path handling and SQLite connection/migration reuse.
- [x] 1.4 Keep application services generic and construct concrete production service instances through native app state methods.

## 2. Command Handler Refactor

- [x] 2.1 Update Provider Setup command handlers to receive managed native app state and obtain services/coordinators through it.
- [x] 2.2 Update Workspace Setup command handlers to receive managed native app state and remove command-local service wiring.
- [x] 2.3 Remove command-local app data path resolution and SQLite Workspace Catalog connection/migration from workspace handlers.
- [x] 2.4 Preserve existing command DTO mapping, generated binding surface, and UI-safe error mapping.

## 3. Behavior Coverage

- [x] 3.1 Add or update native tests proving Workspace Catalog access still returns existing command errors on storage/catalog failures.
- [x] 3.2 Add or update native tests proving provider setup serialization still covers setup, delete, and Workspace creation interactions.
- [x] 3.3 Add or update native tests proving repeated Workspace Catalog command access uses shared managed catalog access after initialization.
- [x] 3.4 Confirm existing command contract tests continue to pass without generated TypeScript contract changes.

## 4. Verification

- [x] 4.1 Run `cargo test`.
- [x] 4.2 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 4.3 Run `cargo fmt`.
