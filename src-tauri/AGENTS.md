# AGENTS.md


## Native Backend Scope

- This directory contains the Tauri native backend, Rust domain/application code, local persistence, secure storage, provider integrations, generated command bindings export, and desktop app configuration.
- Keep Tauri commands as adapters. Commands accept typed requests, call application-layer services, map errors, and return generated binding-safe responses.
- Keep business workflows in application services, not command handlers.
- Keep domain models and rules independent from Tauri runtime APIs, command handlers, UI concerns, persistence adapters, and provider SDK details.

## Secrets

- Keep raw secrets and bearer tokens behind secure storage and provider-call paths.
- Do not include raw provider API keys, worker tokens, Hugging Face keys, or future credentials in domain snapshots, command responses, generated frontend types, logs, metadata, persisted workspace JSON, or test fixtures.

## Diagnostics And Bug Triage

1. Locate logs at `~/Library/Logs/com.luma-forge/luma-forge.log.YYYY-MM-DD`.
2. If the user provides `diagnosticId`, search that exact ID first.
3. Resolve the log date from the user report. If unclear, search today, yesterday, then all `luma-forge.log.*`.
4. If there is no `diagnosticId`, ask for error text, approximate time, workspace ID, and the action that failed.
5. Read the matched log entry, then trace the failure path from its command or lifecycle operation context.
6. Do not diagnose from the UI message alone when log context is available.

## Generated Contracts

- If command signatures or Specta-exported types change, run `bun run codegen:commands` from the repository root.
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
