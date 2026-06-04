# AGENTS.md

## Overview

LumaForge is a desktop application that helps a user provision remote GPU infrastructure and run ComfyUI workflows on that infrastructure.

---

## Project Structure

- `src/`: React frontend. See `src/AGENTS.md`.
- `src-tauri/`: Active minimal Tauri native backend refactor shell. See `src-tauri/AGENTS.md`.
- `src-tauri-legacy/`: Archived previous Tauri native backend, kept for refactor reference.
- `workers/`: Python workers and worker contract tooling. See `workers/AGENTS.md`.
- `bundled/`: Bundled workflow catalog, endpoint contracts, and provisioner contracts.
- `spec/`: Product flows, reference contracts, architecture notes, and ubiquitous language.

---

## Nested Instructions

Additional local instructions live in nested `AGENTS.md` files:

- `src/AGENTS.md`: React frontend.
- `src-tauri/AGENTS.md`: Active Tauri native backend refactor shell.
- `workers/AGENTS.md`: Python workers.

When editing files under those directories, follow both this root file and the nearest nested `AGENTS.md`.

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

### Python Workers

- Own remote environment preparation and runtime workflow execution inside provider-managed compute.
- Follow worker contracts consumed by the Native Layer and bundled catalogs.
- Return only UI-safe progress, status, and result data to the Native Layer.
- Must not persist, log, or re-expose secrets unless explicitly required by an approved spec.

---

## General Rules

### Security and Secrets

- Keep secrets, bearer tokens, provider API keys, worker tokens, Hugging Face keys, and future credentials behind secure storage and trusted provider-call paths.
- Never expose raw credentials to the React renderer, generated frontend types, domain snapshots, command responses, logs, metadata, persisted workspace JSON, test fixtures, or error payloads.
- Treat credentials as write-only from the UI perspective: the frontend may request that a credential exists, is updated, deleted, or validated, but must not receive the raw value back.

### Pre-v1 Refactoring Policy

- LumaForge is not in production yet.
- During refactoring, do not add or preserve legacy fallback paths, compatibility shims, deprecated behavior branches, migration layers, or silent fallback behavior for old contracts.
- Prefer updating all callers, tests, fixtures, and docs to the current contract.
- Do not add tests or assertions for removed functionality, removed fields, legacy vocabulary, or absence of deprecated behavior.
- If old behavior is intentionally removed, delete it directly instead of preserving compatibility code.
- Spec requirements must describe the current contract directly. Do not add spec scenarios or assertions whose purpose is to guard against deprecated behavior, removed fields, legacy vocabulary, or the absence of old behavior.

### Simplicity First

Minimum code that solves the problem. Nothing speculative.

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

### Surgical Changes

Touch only what you must.

- Don't improve adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- Every changed line should trace directly to the user's reques

---

## Commit Conventions

All commits must follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) v1.0.0, for example `feat(scope): description`, `fix(scope): description`, or `docs(scope): description`.
