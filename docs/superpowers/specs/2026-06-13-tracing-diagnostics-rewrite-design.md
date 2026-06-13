# Tracing Diagnostics Rewrite Design

## Context

The native backend already uses `tracing`, `tracing-appender`, and
`tracing-subscriber`. It writes daily JSON logs to the Tauri app log directory,
emits `diagnosticId` values for UI-safe command and lifecycle failures, and
contains redaction helpers for known secret field names.

The rewrite replaces the current manual command log scope pattern with
span-oriented tracing helpers and targeted `#[tracing::instrument]` usage. The
external diagnostics contract stays unchanged.

## Goals

- Keep native diagnostics in `~/Library/Logs/com.luma-forge/luma-forge.log.YYYY-MM-DD`
  on macOS.
- Preserve UI-safe error responses and `diagnosticId` behavior.
- Make command and lifecycle logs easier to trace through operation spans.
- Avoid embedding ad hoc start/completion logging throughout command bodies.
- Keep raw provider keys, bearer tokens, worker tokens, Hugging Face keys, and
  secret-like request data out of logs, spans, events, tests, and fixtures.

## Non-Goals

- No remote telemetry export.
- No OpenTelemetry integration.
- No frontend command contract changes.
- No compatibility layer for the current internal `CommandLogScope` shape.
- No broad instrumentation of every native function.

## Architecture

The diagnostics module owns logging initialization, diagnostic ID generation,
safe span field helpers, error logging, and redaction. Commands and lifecycle
orchestration use diagnostics helpers instead of constructing raw log entries
themselves.

The subscriber remains a single process-wide `tracing` subscriber. File logging
continues to use a non-blocking daily rolling appender. If the app log directory
cannot be created, diagnostics fall back to stdout/stderr formatting as they do
today.

## Command Tracing

Each Tauri command creates a command span with:

- `command`
- safe request metadata
- elapsed duration on completion or failure
- `diagnostic_id` only when a failure is returned

Secret-bearing request fields remain excluded from command metadata. The setup
API key commands record only the command name and timing, not the submitted key.

## Lifecycle Tracing

Lifecycle operations create spans with:

- `operation_id`
- `workspace_id` when available
- operation kind: `provision`, `cleanup`, or `delete`
- current lifecycle step when available
- lifecycle state on failure

Failure logs include `diagnostic_id`, error code, redacted leaf error message,
and source-chain error codes. Normal lifecycle events remain allowed to use
`diagnosticId: null` in frontend-visible events.

## Error Handling

Typed domain/application errors remain authoritative. Diagnostics convert errors
to log fields and UI-safe `NativeCommandError` values at command and lifecycle
boundaries.

The rewrite keeps the current leaf-error behavior for UI messages and logs. Log
messages pass through redaction before being emitted.

## Testing

Tests cover:

- secret-bearing command requests do not produce metadata
- safe command requests produce expected metadata
- lifecycle payloads map to stable span fields
- redaction handles known secret field names
- command/lifecycle error conversion preserves diagnostic IDs and safe fields

The implementation follows TDD for changed behavior.

## Verification

Run from the repository root:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

