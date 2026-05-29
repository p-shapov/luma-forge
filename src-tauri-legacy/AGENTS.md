# AGENTS.md

## Native Backend Scope

- This directory contains the Tauri native backend, Rust domain/application code, local persistence, secure storage, provider integrations, generated command bindings export, and desktop app configuration.
- Keep Tauri commands as adapters. Commands accept typed requests, call application-layer services, map errors, and return generated binding-safe responses.
- Keep business workflows in application services, not command handlers.
- Keep domain models and rules independent from Tauri runtime APIs, command handlers, UI concerns, persistence adapters, and provider SDK details.

---

## Structure

- `src/commands/`: Tauri command adapters and Specta binding export.
- `src/domain/`: Domain models, validation, and rules.
- `src/provider_setup/`: Provider API key validation and secure setup workflow.
- `src/workspace_setup/`: Draft Workspace creation from catalogs and placement input.
- `src/workspace_provisioning/`: Provisioning lifecycle orchestration, progress sync, cancellation, and failure handling.
- `src/workspace_resources/`: Provider resource lifecycle, naming, state, and cleanup.
- `src/secrets/`: Secure secret storage abstraction and keyring implementation.
- `src/workspace_catalog/`: SQLite-backed Workspace Catalog persistence.
- `src/bundled_catalog/`: Bundled catalog loading and validation.
- `capabilities/`: Tauri capability declarations.

---

## Secrets

- Keep raw secrets and bearer tokens behind secure storage and provider-call paths.
- Do not include raw provider API keys, worker tokens, Hugging Face keys, or future credentials in domain snapshots, command responses, generated frontend types, logs, metadata, persisted workspace JSON, or test fixtures.

---

## Generated Contracts

- If command signatures, command request/response types, or Specta-exported types change, run `bun run codegen:commands` from the repository root.
- Do not manually edit `../src/generated/commands.ts`.

---

## Verification

For native backend changes, run from the repository root:

- `cargo test --manifest-path src-tauri/Cargo.toml`
- `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`

If Tauri command contracts change, also run:

- `bun run codegen:commands`
- `bun run build`
- `bun run lint`
