# Native Operation Tracing Design

## Context

LumaForge needs native operation tracing for the Tauri backend. The first implementation covers the whole tracing brief: command boundaries, workspace lifecycle operations, detached lifecycle runners, and RunPod provider runtime work.

The design uses `fastrace` for span structure and `log`/`logforth` for operational diagnostics. Spans provide operation shape and duration. JSONL logs provide point-in-time diagnostics that can be searched by trace ID.

## Goals

- Initialize native logging and tracing after support paths are ready.
- Write JSONL logs under `<app_data_dir>/logs/luma-forge.log`.
- Create one root span for every Tauri command.
- Derive UI-safe command `trace_id` values from `fastrace::collector::SpanContext`.
- Use fastrace-derived command error trace IDs as the current native contract.
- Keep ordinary service and provider method contracts free of tracing parameters.
- Continue command traces across detached lifecycle background work.
- Add safe operational diagnostics for workspace lifecycle and RunPod runtime milestones.
- Keep secrets, tokens, command payloads, request bodies, raw provider responses, and large responses out of logs and span properties.

## Non-Goals

- Do not add trace IDs or `SpanContext` parameters to ordinary service/provider method signatures.
- Do not use `Span::add_event`, `LocalSpan::add_event`, `Event::add_to_parent`, or `Event::add_to_local_parent`.
- Do not add command/service/provider tests whose only purpose is proving instrumentation exists.

## Dependencies

Add native dependencies:

- `fastrace = { version = "0.7", features = ["enable"] }`
- `log`
- `logforth` with JSON file output and fastrace diagnostics support

The implementation must use `FutureExt::in_span(...)` for async command and detached lifecycle futures so tracing context is preserved across async execution.

## Module Boundaries

Add a top-level `diagnostics` module under `src-tauri/src`. It is infrastructure, not app bootstrap code.

`diagnostics` owns generic primitives only:

- initialize logging/tracing from a logs directory path
- build the JSONL log file path
- format a `SpanContext` trace ID for UI/support use
- read the current local parent trace ID when needed
- extract safe error `code`, `message`, `cause`, and `source_chain` details

`diagnostics` must not know about Tauri commands, command error enums, workspace services, provider types, or app bootstrap types.

`app` remains responsible for support paths and state bootstrap. After `prepare_support_paths` succeeds, Tauri setup calls diagnostics initialization with `support_paths.logs_dir()` before building app state.

`tauri_api` owns command-specific wrappers:

- create command root spans by command name
- run command futures under those spans
- derive command error trace IDs
- map and log command boundary errors with public command error codes

`workspace` owns lifecycle semantics. It captures trace context at the lifecycle spawn boundary and moves only that captured context into detached work.

`provider::runpod` owns safe provider operation diagnostics for RunPod milestones.

## Startup Flow

1. Tauri setup mounts generated events.
2. `prepare_support_paths` creates the app data directory, logs directory, and database path.
3. diagnostics initialization configures `logforth` JSONL output and fastrace reporting.
4. app state bootstrap runs after diagnostics are active.
5. bootstrap and stale lifecycle recovery run under startup/recovery spans so their logs have trace context.

If support paths cannot be prepared, file logging cannot be initialized. That failure still returns the existing native initialization command error shape.

## Command Flow

Each Tauri command adapter creates an explicit command root span. The adapter derives `SpanContext::from_span(&root)` and uses `span_context.trace_id` as the UI-safe `CommandError.trace_id`.

The command's async work runs under the command span using `FutureExt::in_span(root)`. Ordinary calls into workspace services, secret services, catalogs, and providers rely on the active local parent context. Their method signatures do not gain tracing arguments.

`CommandError.trace_id` remains part of the UI/support contract. Only its source changes: fastrace span context replaces the current random UUID helper.

Command boundary logs should include safe fields such as command name, public error code, workspace ID, and operation ID when those values are already safe and available. They must not include command payloads or secrets.

## Detached Lifecycle Flow

Command-driven lifecycle work must continue the command trace.

Lifecycle entry methods such as `provision_workspace`, `cleanup_workspace`, and `delete_workspace` are annotated with `#[fastrace::trace]`. They call `spawn_lifecycle_runner()` directly after creating the durable lifecycle operation.

Inside `spawn_lifecycle_runner()`, before `tokio::spawn`, capture `SpanContext::current_local_parent()`. This happens while the lifecycle entry method span is active, so no diagnostic-only action parameter is needed.

Move that captured `Option<SpanContext>` into the spawned task. Inside the task:

- create a generic lifecycle runner span from the captured parent when present
- use a startup/recovery parent for recovery paths when available
- fall back to a new root only when there is genuinely no parent context
- run the detached future under that lifecycle span with `FutureExt::in_span(...)`

RunPod runtime/provider spans and logs inherit from the lifecycle span through the active local parent context. No ordinary runtime method accepts `trace_id` or `SpanContext`.

## UI Event Sink

`WorkspaceEventSink::emit` is for UI-facing event correlation only.

It must not:

- create spans or roots
- write logs
- write dev-log records
- accept raw tracing context as a general contract

If a UI event later needs correlation, pass a precomputed UI-safe trace ID as event data. Do not pass `SpanContext` into the event sink.

## Logging Model

Use `log::{info, warn, error}` for operational diagnostics. Configure `logforth` so each JSONL record includes fastrace context from the active span:

- `trace_id`
- `span_id`
- `level`
- `target`
- `message`
- safe structured fields

Records emitted without an active span are still written, but have no trace context.

Log milestone coverage:

- command start, success, and failure
- app bootstrap and stale lifecycle recovery
- workspace lifecycle start, duplicate-suppressed spawn, completion, and failure
- RunPod network volume create/delete
- RunPod provisioner pod start, status polling, and termination
- RunPod serverless template create/delete
- RunPod serverless endpoint create/delete

Provider request diagnostics are operation-level only. RunPod provider operation failures should log the original `RunpodProviderError` before it is mapped into a `WorkspaceError`, using the same safe `code`, `message`, `cause`, and `source_chain` fields. Do not log raw request bodies, response bodies, auth headers, signed URLs, or large responses.

## Error Diagnostics

Log errors once at the boundary that finalizes the operation outcome.

Command failures are logged at the command boundary. Detached lifecycle failures are logged at the lifecycle boundary that marks the operation failed. RunPod provider operation failures are logged at the provider operation boundary before provider errors are converted into workspace errors. Other lower layers return errors without duplicate logs unless they own a finalized outcome.

Failure diagnostics use safe fields:

- `code`: public command error enum or provider/domain error kind
- `message`: UI-safe top-level message
- `cause`: leaf source error message
- `source_chain`: ordered source chain from immediate source to leaf

`code` must come from typed error mapping or a serialized externally tagged domain/provider error enum, not ad hoc raw string literals. Error diagnostics may inspect the serialized shape to read the variant tag, but must not log the serialized payload. `message` is the top-level error display string. Unsafe source-chain links must be omitted rather than sanitized into misleading text.

## Redaction Rules

Never log or attach:

- secrets
- bearer tokens
- provider API keys
- worker tokens
- Hugging Face keys
- auth headers
- signed URLs
- command payloads
- sensitive payloads
- request bodies
- raw provider response bodies
- large provider responses

Macro properties must come from a fixed allowlist. Do not format arbitrary function arguments into macro properties.

## Testing

Keep new tests local to the new `diagnostics` module and only test diagnostics-owned logic that is non-trivial.

Allowed tests:

- safe `code`, `message`, `cause`, and `source_chain` extraction for nested externally tagged enum errors
- redaction or omission behavior when diagnostics filters unsafe source-chain entries
- log filename/date path helper if implemented as standalone logic

Do not add tests for:

- `SpanContext::from_span`
- trace ID formatting
- fastrace/logforth library behavior
- command/service/provider instrumentation wiring

Native verification must run:

- `cargo test --manifest-path src-tauri/Cargo.toml`
- `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`

Run command codegen and frontend checks only if command signatures or generated Specta types change.

## Acceptance Criteria

- JSONL logs are written to `<app_data_dir>/logs/luma-forge.log`.
- Logs emitted under active spans include `trace_id` and `span_id`.
- Every Tauri command creates one command root span.
- Every command error returns a fastrace-derived `trace_id`.
- Ordinary service/provider method signatures do not gain tracing parameters.
- Command-driven detached lifecycle work remains in the same trace as the command.
- Startup stale-operation recovery has trace context.
- Workspace lifecycle and RunPod runtime milestones emit safe operational diagnostics.
- No logs or span properties expose secrets, credentials, auth material, command payloads, request bodies, raw provider responses, or large responses.
