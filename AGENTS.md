# AGENTS.md

## Overview

LumaForge is a macOS desktop application that helps a user provision remote GPU infrastructure and run ComfyUI workflows on that infrastructure.

The local app provides the UI and orchestration surface. ML inference does not run locally. In v1, RunPod is the only supported GPU Cloud Provider, and the Tauri native layer communicates with RunPod directly; LumaForge does not depend on its own hosted backend service.

---

## Platforms

macOS only

---

## Runtime Responsibilities

### React Frontend

- Owns presentation, navigation, user interaction, and temporary UI state.
- Delegates durable state changes, provider access, and local system access to the Native Layer.
- Treats Native Layer responses as authoritative for persisted state and long-running operations.
- Must not persist, log, or re-expose secrets after submission.

### Tauri Native Layer

- Owns local system integration, secure storage, provider communication, durable state, and authoritative validation.
- Coordinates operations that mutate local or provider-owned resources.
- Returns only UI-safe data to React.
- Ensures durable state is consistent before reporting success.

---

## Critical Flows

- [GPU Cloud Provider Setup](./spec/flows/gpu-cloud-provider-setup.md): validates one provider key, stores it in the secure keyring, then persists the selected provider. Repeated setup is rejected after a complete setup exists.
- [Workspace Setup](./spec/flows/workspace-setup.md): creates one local `Draft` Workspace Catalog entry from a Workflow Preset and Placement Plan. It does not create Provider Resources.
- [Workspace Provisioning](./spec/flows/workspace-provisioning.md): provisions one saved `Draft` Workspace into `Ready` by creating Provider Resources, preparing the environment, syncing progress, and preserving cleanup metadata on failure.

---

## Tech Stack (as of April 2026)

### Frontend

- React 19
- TypeScript
- Vite
- Bun
- TanStack Router
- Zustand
- shadcn/Radix-style UI primitives
- Tailwind CSS

### Backend (Tauri)

- Tauri 2
- Rust
- Tokio
- Reqwest
- SQLx with SQLite
- macOS keyring integration via `keyring`
- `secrecy` / `zeroize` for secret handling
- `specta` / `tauri-specta` for generated TypeScript command bindings

---

## Architecture Principles

### React Frontend

- Keep React as a thin client over Native-owned domain state. React composes screens, collects input, renders status, and calls Native commands.
- Treat data returned from Native commands as authoritative. Do not patch persisted domain state locally after Native-owned operations.
- Keep temporary UI state in React, such as user input, selections, filters, loading states, and local navigation progress.
- Do not encode Native-owned workflow decisions in React. React may trigger operations and render returned state, but Native Layer owns durable decisions and side effects.
- Keep frontend structure page-first. Use `app/`, `pages/`, and `shared/` by default; add `features/`, `entities/`, or `widgets/` only when reuse or complexity makes the extraction worthwhile.
- Use `features/` for reusable or independently complex user actions, not just because code contains domain terms.
- Keep `shared/` infrastructure-only: UI primitives, generic utilities, generated command access, and non-domain helpers.

**Verification Criteria**

For any changes in `src/`:

- Run `bun run build`
- Run `bun run lint --fix`

If frontend test or type-check scripts are added later, run them for relevant frontend changes.

### Tauri Backend

- Keep Tauri commands as adapters. Commands accept typed requests, call application-layer code, map errors, and return generated binding-safe responses.
- Keep business workflows out of command handlers. Application-layer services coordinate validation, state changes, provider calls, transactions, and error handling.
- Keep domain models and rules independent from Tauri runtime APIs, command handlers, UI concerns, and provider-specific SDK details.
- Keep side effects behind infrastructure modules: persistence, keyring access, filesystem/config access, HTTP clients, logging, and bundled/generated catalog loading.
- Keep provider-specific request/response shapes, naming, authentication details, API quirks, and mapping code inside provider implementation modules.
- Treat durable native state as Native-owned. Native code defines persistence and consistency boundaries; React consumes the resulting state through commands.
- Keep secrets behind secure storage and provider-call paths. Secrets must not appear in domain snapshots, command responses, logs, diagnostics, or generated frontend types.
- Treat generated command bindings as the frontend contract. Request and response types should be exported through `specta` / `tauri-specta` instead of duplicated by hand.

**Verification Criteria**

For any changes in `src-tauri/`:

- Run `cargo test`
- Run `cargo clippy --fix --allow-dirty --allow-staged`
- Run `cargo fmt`

---

## Commit Conventions

All commits **must** follow the [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) specification (v1.0.0).

### Message Format

```
<type>[optional scope]: <description>
[optional body]
[optional footer(s)]
```

### Allowed Types

- `feat` — new feature
- `fix` — bug fix
- `docs` — documentation changes
- `style` — formatting, whitespace
- `refactor` — code refactoring
- `perf` — performance improvement
- `test` — adding or updating tests
- `build` — build system or dependencies
- `ci` — CI/CD configuration
- `chore` — other changes
- `revert` — revert previous commit

### Scope (recommended)

`frontend`, `tauri`, `onboarding`, `image-generation`, `lora`, etc. (kebab-case).

### Rules

- Commit messages **must be in English**
- Subject line ≤ 72 characters
- Use imperative mood (“add”, “fix”, “update”)
- Breaking changes: add `!` after type/scope + `BREAKING CHANGE:` in the footer
- One logical change per commit
