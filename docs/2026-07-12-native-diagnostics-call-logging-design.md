# Native Diagnostics And Call Logging Design

## Context

The active native refactor under `src-tauri/src` needs one trace to begin at each future Tauri command and remain available through application services, port implementations, adapters, infra clients, and detached background work. It also needs a systematic logging rule instead of selectively instrumented milestones.

The existing `src-tauri/old_src/diagnostics` implementation is a reference for `fastrace`, `logforth`, JSONL output, error diagnostics, and secret sanitization. This design reuses its proven security behavior where useful, but does not preserve its module APIs or custom error-code machinery when a simpler current implementation is sufficient.

This document is the current diagnostics design for `src-tauri/src`. It supersedes the older active-code recommendations in `docs/superpowers/specs/2026-06-21-native-operation-tracing-design.md` where the two differ.

## Goals

- Create a root `fastrace` context at every entry point that does not already have one.
- Propagate tracing through ordinary calls without adding trace parameters to function signatures.
- Preserve tracing explicitly across background boundaries such as `tokio::spawn`.
- Persist the command trace ID in each runtime operation.
- Write `start`, `success`, or `error` records for every public runtime operation.
- Log sanitized inputs and outputs at those public boundaries.
- Keep secrets, request bodies, raw provider responses, signed URLs, and oversized diagnostics out of the log file.
- Centralize call logging so the rule has one implementation.

## Non-Goals

- Do not pass `TraceId` or `SpanContext` through ordinary service, port, adapter, or client signatures.
- Do not instrument constructors, getters, private helpers, pure mapping functions, or trait declarations without executable bodies.
- Do not add a custom diagnostics proc macro or a custom lint.
- Do not add a tracing exporter or external telemetry backend.
- Do not catch panics or convert them into application errors.
- Do not implement future Tauri commands as part of this diagnostics change.

## Diagnostics Boundary

Add a top-level `diagnostics` module under `src-tauri/src`. It owns:

- `logforth` initialization;
- the sanitizing JSON layout and application log filter;
- current `fastrace` context access;
- async and synchronous call wrappers;
- safe error-chain extraction.

The module may reuse and adapt the old `diagnostics` files, especially the sanitization rules and their security tests. The active implementation must be shaped around the current `application`, `adapters`, and `infra` boundaries rather than copied wholesale.

Diagnostics initialization happens after support paths and the log directory exist, but before application state is constructed or application operations run. The output remains one JSONL file at `<app_data_dir>/logs/luma-forge.log`.

## Trace Propagation

`diagnostics::run` and `diagnostics::run_sync` own span creation:

- with an active local parent, the wrapper creates a child span;
- without an active local parent, the wrapper creates a root span from `SpanContext::random()`;
- start and terminal logs are emitted while that span is active;
- nested public calls therefore inherit the current trace and create nested spans without parameter drilling.

Future Tauri command adapters use the same wrapper. While the wrapper is active, a command can read `diagnostics::current_trace_id()` when it needs to construct a UI-safe command error.

### Background tasks

`tokio::spawn` does not implicitly inherit the active local parent. Before spawning, the caller captures `SpanContext::current_local_parent()`. The spawned future creates a runner span with that captured context and executes through `FutureExt::in_span`. Public operations inside the task then return to the ordinary wrapper rule.

The same explicit propagation rule applies to other execution boundaries that do not preserve the active future context, including `spawn_blocking` and future process or HTTP trace propagation.

### Runtime operations

`RuntimeOperation.trace_id` changes from `Uuid` to `fastrace::collector::TraceId`. Operation IDs remain `Uuid`.

Provision and cleanup stop generating a separate random trace ID. They read the active trace ID when the durable operation is created. SQLite stores the canonical `TraceId` string and parses that type when operations are loaded.

During startup recovery, each interrupted operation is handled under a new recovery span created with its stored trace ID. This reconnects recovery logs to the original support correlation ID even though the pre-restart span no longer exists.

## Public Call Contract

The diagnostics module exposes two common wrappers:

```rust
diagnostics::run(name, input, future).await
diagnostics::run_sync(name, input, operation)
```

For a fallible public operation, the wrapper:

1. creates and activates the function span;
2. emits `INFO call.start` with `function` and `input`;
3. executes the supplied future or synchronous operation;
4. emits exactly one terminal record;
5. emits `INFO call.success` with `function` and `output` for `Ok`;
6. emits `ERROR call.error` with `function` and safe error diagnostics for `Err`;
7. returns the original result unchanged.

The function name is a static logical name such as:

- `application.workspace.create`;
- `adapters.sqlite.workspace_repository.create`;
- `infra.runpod.create_network_volume`.

Input, output, and error values use `Debug` so domain and application types do not gain `Serialize` solely for diagnostics. Secret-bearing values must use redacting types such as `SecretString`; the final layout then applies the generic body, size, key, and token sanitization rules.

A panic is not a `Result::Err`; the wrapper does not catch unwinding. A panicking operation may therefore leave only its `call.start` record.

## Instrumentation Rule

Every executable public runtime operation uses `diagnostics::run` or `diagnostics::run_sync`:

- future Tauri commands;
- public application service methods;
- executable application port implementations;
- public adapter operations;
- public infra client operations.

The following are excluded:

- constructors;
- getters and accessors;
- private helpers;
- pure mapping functions;
- trait declarations without executable bodies.

Errors are intentionally logged at every public boundary they cross. The infra client records the original network or decoding error, the adapter records its mapped port error, and the application service records its application error. Their shared trace ID correlates the records.

A detached operation has no waiting public caller to record its eventual failure. The private background boundary that finalizes a runtime operation therefore emits one additional `call.error`-shaped operation failure record before persisting the failed state. This is the only instrumentation exception for a private function; it prevents background persistence, transition, and orchestration failures from disappearing after the initiating public method has already returned success.

The rule is added to `src-tauri/AGENTS.md`. A custom lint and proc macro are deferred because the common wrapper plus a complete implementation audit provides the rule without another maintenance subsystem.

## Log Format

`logforth` writes application records at `INFO` and above. `FastraceDiagnostic` adds the active `trace_id` and `span_id` to each wrapper record.

A successful terminal record has this conceptual shape:

```json
{
  "timestamp": "2026-07-12T00:00:00Z",
  "level": "INFO",
  "target": "luma_forge::diagnostics",
  "message": "call.success",
  "ctx": {
    "function": "application.workspace.create",
    "output": "Workspace { ... }"
  },
  "diags": {
    "trace_id": "0123456789abcdef0123456789abcdef",
    "span_id": "0123456789abcdef"
  }
}
```

The start record contains `input` instead of `output`. An error record lifts a sanitized structured error object to the top-level `error` field.

## Error Diagnostics

Diagnostics uses the standard `std::error::Error::source()` chain. It does not restore the old `HasDiagnosticCode` trait or the `luma-diagnostic` proc-macro crate.

An error record contains:

- the Rust error type name;
- sanitized `Debug` output;
- sanitized top-level `Display` message;
- sanitized source-chain messages in source order.

Each current boundary already has a typed error enum. Logging each boundary before or after its mapping preserves the concrete lower-layer failure without requiring a parallel diagnostics error taxonomy. Stable public error codes remain the responsibility of future command response contracts, not the generic logging module.

## Sanitization

`SanitizingJsonLayout` is the final mandatory barrier before a record reaches disk. It recursively sanitizes messages, inputs, outputs, error diagnostics, context values, and diagnostic values.

The active implementation retains or improves the old security behavior:

- secret-bearing keys, bearer tokens, API keys, authorization values, and other credentials become `[REDACTED]`;
- Hugging Face tokens and signed URL credentials are redacted inside strings;
- body-like values become `[REDACTED_BODY]`;
- values above the diagnostic size limit become `[REDACTED_LARGE_DIAGNOSTIC]`;
- nested structured values are sanitized recursively;
- serialization or formatting failures produce a fixed safe fallback rather than the original value.

`SecretString` remains the required type for credentials crossing public operations. Its redacted `Debug` representation is the first protection; the final layout remains defense in depth for raw strings and incorrectly shaped diagnostic values.

Raw HTTP request bodies and raw provider response bodies are not public-operation diagnostic values. Public infra clients log their typed, sanitized inputs and outputs after transport mapping. Large typed outputs are suppressed by the same size policy.

## Testing

Diagnostics-owned tests cover:

- async `start -> success`;
- async `start -> error`;
- the equivalent synchronous wrapper behavior;
- standalone root creation and nested child-span behavior;
- secret-bearing keys and nested values;
- bearer tokens, API keys, authorization values, Hugging Face tokens, and signed URLs;
- body-like and oversized diagnostic suppression;
- error message and standard source-chain sanitization.

Existing runtime operation tests change their trace fixtures and assertions from `Uuid` to `fastrace::TraceId`. No instrumentation-only test is added for each service or adapter method.

Implementation includes a complete search-based audit of executable public operations under `src-tauri/src/application`, `src-tauri/src/adapters`, and `src-tauri/src/infra` to apply the wrapper consistently.

Native verification runs:

```sh
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Command code generation and frontend verification are required only when future Tauri command signatures or generated binding-safe types change.

## Acceptance Criteria

- Every executable public runtime operation in scope emits one `call.start` and exactly one `call.success` or `call.error` for normal `Result` completion.
- Nested operations share one trace ID and have nested span contexts without trace parameters in their signatures.
- Detached background work continues the originating trace through explicit `SpanContext` capture.
- Runtime operations persist the active `fastrace::TraceId`; provision and cleanup do not generate separate trace IDs.
- Startup recovery logs use each interrupted operation's stored trace ID.
- Inputs, outputs, and errors are present in the appropriate call records and are sanitized before disk output.
- Secrets, credentials, body-like values, signed URL credentials, and oversized diagnostics never appear raw in `luma-forge.log`.
- The instrumentation rule is documented in `src-tauri/AGENTS.md`.
- Native tests, formatting, and Clippy pass.
