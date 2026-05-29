# AGENTS.md

## Native Backend Scope

- This directory is the active minimal Tauri shell for the native backend refactor.
- Keep temporary code small and explicit.
- Do not restore legacy native workflows here; use `../src-tauri-legacy` only as reference.

## Secrets

- Do not persist, log, return, or expose raw provider API keys, Hugging Face keys, worker tokens, or future credentials.

## Generated Contracts

- If command signatures or Specta-exported types change, run `bun run codegen:commands` from the repository root.
- Do not manually edit `../src/generated/commands.ts`.

## Verification

For active native shell changes, run from the repository root:

- `cargo test --manifest-path src-tauri/Cargo.toml`
- `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`
