# AGENTS.md

## Overview

LumaForge is a desktop application that helps a user provision remote GPU infrastructure and run ComfyUI workflows on that infrastructure.

---

## Critical Flows

- [GPU Cloud Provider Setup](./spec/flows/gpu-cloud-provider-setup.md): validates one provider key, stores it in the secure keyring, then derives setup status from the stored key and provider identity. Repeated setup is rejected after a complete setup exists.
- [Workspace Setup](./spec/flows/workspace-setup.md): creates one local `Draft` Workspace Catalog entry from a Workflow Preset and Placement Plan. It does not create Provider Resources.
- [Workspace Provisioning](./spec/flows/workspace-provisioning.md): provisions one saved `Draft` Workspace into `Ready` by creating Provider Resources, preparing the environment, syncing progress, and preserving cleanup metadata on failure.

---

## Architecture Principles

### React Frontend

- Own presentation, navigation, user interaction, and temporary UI state.
- Keep React as a thin client over Native-owned domain state. React composes screens, collects input, renders status, and calls Native commands.
- Do not encode Native-owned workflow decisions in React. React may trigger operations and render returned state, but Native Layer owns durable decisions and side effects.
- Treat Native Layer responses as authoritative for persisted state and long-running operations.
- Keep `shared/` infrastructure-only: UI primitives, generic utilities, generated command access, and non-domain helpers.

### Tauri Backend

- Own local system integration, secure storage, provider communication, durable state, and authoritative validation.
- Coordinate operations that mutate local or provider-owned resources.
- Ensure durable state is consistent before reporting success.
- Keep Tauri commands as adapters. Commands accept typed requests, call application-layer code, map errors, and return generated binding-safe responses.
- Keep business workflows out of command handlers. Application-layer services coordinate validation, state changes, provider calls, transactions, and error handling.
- Keep domain models and rules independent from Tauri runtime APIs, command handlers, UI concerns, and provider-specific SDK details.

### Workers

- Keep worker runtime contracts explicit and compatible with the bundled catalogs and native provisioning flow.
- Treat `workers/provisioner/` as the container-side workspace preparation worker.
- Treat `workers/runpod-endpoint/` as the RunPod Serverless runtime worker for prepared ComfyUI environments.

### Secrets

- Keep secrets and bearer tokens behind secure storage and provider-call paths.
- Do not include raw provider API keys, worker tokens, Hugging Face keys, or future credentials in domain snapshots, command responses, generated frontend types, logs, metadata, persisted workspace JSON, or test fixtures.

---

## Generated Files

- Do not manually edit `src/generated/**`.
- If `src/routes/**` changes, run `bun run codegen:routes`.
- If Tauri command signatures, command request/response types, or Specta-exported types change, run `bun run codegen:commands`.
- After generated frontend contracts change, run `bun run build` and `bun run lint`.

---

## Verification

For frontend changes in `src/`, run:

- `bun run build`
- `bun run lint`

For backend changes in `src-tauri/`, run:

- `cargo test --manifest-path src-tauri/Cargo.toml`
- `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`

For provisioner worker changes in `workers/provisioner/`, run:

- `PYTHONPATH=workers/provisioner/src python3 -m unittest discover -s workers/provisioner/tests`

For RunPod endpoint worker changes in `workers/runpod-endpoint/`, run:

- `PYTHONPATH=workers/runpod-endpoint/src python3 -m unittest discover -s workers/runpod-endpoint/tests`

If Tauri command contracts change, also run:

- `bun run codegen:commands`
- `bun run build`
- `bun run lint`

---

## Commit Conventions

All commits must follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) v1.0.0, for example `feat(scope): description`, `fix(scope): description`, or `docs(scope): description`.
