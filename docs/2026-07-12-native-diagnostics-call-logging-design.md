# Native Diagnostics And Call Logging Design

## Context

The active native refactor under `src-tauri/src` needs one trace to begin at each future Tauri command and remain available through application services, production port implementations, adapters, infra clients, and detached background work. Logging must follow one systematic rule instead of relying on hand-written start and outcome records.

The diagnostics implementation is new code designed for the active architecture. `src-tauri/old_src/diagnostics` is not an implementation base, and its wrappers, custom JSON layout, sanitizer, error traits, tests, and proc macros are not copied into active code.

This document supersedes the earlier contents of this file and the active-code recommendations in `docs/superpowers/specs/2026-06-21-native-operation-tracing-design.md` where they differ.

## Goals

- Create one root `fastrace` context at every true native entry point.
- Propagate tracing through ordinary calls without trace parameters in function signatures.
- Make every instrumented operation safe to call without a pre-existing span by creating a root when no parent exists.
- Preserve tracing across detached execution boundaries inside the diagnostic macro rather than application function bodies.
- Persist the active trace ID as `Option<Uuid>` in each runtime operation.
- Emit `start`, `success`, and `error` records through a declarative function attribute.
- Make logged values explicit and fail-closed without heuristic sanitization.
- Standardize safe diagnostic formatting for external boundary DTOs.
- Keep secrets, credentials, raw request bodies, raw provider responses, and large payloads out of logs by construction.

## Non-Goals

- Do not pass `TraceId` or `SpanContext` through ordinary service, port, adapter, or client signatures.
- Do not log private helpers, `pub(crate)` module helpers, constructors, getters, pure mappings, domain mutations, or test fakes.
- Do not copy or adapt the old diagnostics module.
- Do not add a wrapper API around every function body.
- Do not add a custom JSON sanitizer or custom log layout.
- Do not traverse arbitrary error source chains.
- Do not add an external trace exporter.
- Do not catch panics or convert them into application errors.
- Do not implement future Tauri commands or their frontend bindings in this change.

## Components

The feature has two focused components.

### Runtime diagnostics module

`src-tauri/src/diagnostics` owns:

- `logforth` initialization;
- the standard `logforth::layout::JsonLayout`;
- `FastraceDiagnostic` integration;
- optional current trace ID capture and conversion between `Uuid` and `fastrace` IDs;
- small formatting helpers used by generated macro code;
- the `DiagnosticValue` marker trait;
- safe `Redacted` and named-field debug views.

It does not own service, adapter, provider, persistence, or Tauri semantics.

### Diagnostics proc-macro crate

A new proc-macro crate under `src-tauri/crates/diagnostics-macros` owns:

- the `#[diagnostic]` function and impl attribute;
- the `#[derive(DiagnosticDebug)]` derive;
- parsing and validation of diagnostic annotations;
- generated logging code;
- root, child, detached, and stored-trace restoration composition with `fastrace`.

The macro crate is implemented from scratch. It uses `syn`, `quote`, and `proc-macro2`, but contains no runtime logger or business-specific logic.

The native crate re-exports the macros from its diagnostics module so active code has one diagnostics namespace.

## Diagnostic Values

### `DiagnosticValue`

Showing a value requires more than an arbitrary `Debug` implementation:

```rust
pub trait DiagnosticValue: std::fmt::Debug {}
```

The runtime module implements this trait for an explicit safe scalar set:

- integer and floating-point primitives;
- `bool` and `char`;
- `str` and `String`;
- `()`;
- `Uuid` and `fastrace::collector::TraceId`;
- `SecretString`, whose own `Debug` is already redacted;
- references, `Option`, `Vec`, slices, arrays, and supported tuples when their contained values implement `DiagnosticValue`.

There is no blanket `impl<T: Debug> DiagnosticValue for T`. A struct or enum with only ordinary `Debug` therefore cannot be shown accidentally.

`serde_json::Value`, generated provider DTOs, transport responses, request builders, and other open-ended values are not diagnostic scalars.

### `DiagnosticDebug`

`#[derive(DiagnosticDebug)]` generates:

- a safe `std::fmt::Debug` implementation;
- an implementation of `DiagnosticValue`.

It supports structs and enums. Fields are fail-closed:

- no annotation: omit the field;
- `#[diagnostic(show)]`: format the field through `DiagnosticValue`;
- `#[diagnostic(redact)]`: keep the field name and render `[REDACTED]` without requiring any formatting trait from the field type.

There is no `skip` annotation because omission is the default.

Example:

```rust
#[derive(DiagnosticDebug)]
pub struct CreatePodRequest {
    #[diagnostic(show)]
    pub workspace_id: String,

    #[diagnostic(show)]
    pub datacenter_id: String,

    #[diagnostic(redact)]
    pub bearer_token: SecretString,

    pub model_assets: serde_json::Value,
}
```

Its diagnostic representation includes `workspace_id`, `datacenter_id`, and a redacted `bearer_token`; `model_assets` is absent.

Deriving ordinary `Debug` and `DiagnosticDebug` for the same type is a compile error because both implement `Debug`.

## External Boundary DTOs

Named request and response structures are required at external boundaries:

- future Tauri command requests and responses;
- public provider and HTTP client requests and responses;
- future worker and process contracts.

These boundary DTOs derive `DiagnosticDebug`. Only explicitly shown fields can appear in logs.

Generated HTTP and GraphQL types remain private wire representations. A public infra client accepts and returns application-owned boundary DTOs, then converts to or from generated types inside its private transport implementation.

The current RunPod pod request is an important cutover case: a generated `PodCreateInput.env` containing bearer and Hugging Face values as raw `String` must not cross a public logged signature. The public client accepts a safe request containing `SecretString`; the raw generated environment map is created only immediately before the private HTTP call.

This DTO rule does not apply to every in-process selector. Public in-process operations may retain named scalar parameters and scalar results, such as `workspace_id: &str`, `limit: u64`, `bool`, one ID, or `()`.

## Function Instrumentation

### Logged operations

Instrumentation applies to executable public operations:

- future Tauri commands;
- public application service methods;
- production implementations of public port traits;
- public infra client methods.

It does not apply to private methods, `pub(crate)` helpers, constructors, getters, pure mapping functions, domain mutations, body-less trait declarations, or test fakes.

Detached task entry points and per-operation interrupted-recovery entry points are the only private exceptions. Each is an execution boundary outside the initiating public method, so it must own its terminal success or failure log. Unrelated private helpers remain uninstrumented.

### Attribute policy

`#[diagnostic]` supports async and synchronous functions that return `Result<T, E>`. It automatically ignores `self` and rejects destructuring parameters so logged argument names remain stable.

Argument values are explicit and fail-closed:

- no parameter annotation: omit the argument;
- `#[diagnostic(show)]`: include the argument and require `DiagnosticValue`;
- `#[diagnostic(redact)]`: include the argument name with `[REDACTED]` and impose no formatting trait on its type.

Example:

```rust
#[diagnostic(show_output, show_error)]
pub async fn create_pod(
    #[diagnostic(show)] request: CreatePodRequest,
    #[diagnostic(redact)] api_key: &SecretString,
    transport_options: InternalOptions,
) -> Result<CreatePodResponse, NetworkError> {
    // operation
}
```

The request and redacted API key are present in `call.start`; `transport_options` is absent.

Output values are also fail-closed:

- no output annotation: omit the `output` field;
- `show_output`: require `T: DiagnosticValue` and show the value;
- `redact_output`: write `output = [REDACTED]`.

The error event is always emitted. It always contains the Rust error type name, but the value is fail-closed:

- no error annotation: omit the error value;
- `show_error`: require `E: DiagnosticValue` and show its safe `Debug`;
- `redact_error`: write `error = [REDACTED]`.

Conflicting annotations are compile errors.

### Generated records

For normal `Result` completion, the macro emits:

1. `INFO call.start` with the function name and explicitly selected named inputs;
2. `INFO call.success` with an explicitly selected or redacted output, when configured;
3. `ERROR call.error` with the error type and an explicitly selected or redacted error value, when `Err` is returned.

It returns the original `Result` unchanged. A panic is not caught and may leave only `call.start`.

The runtime uses the standard `logforth` JSON layout. There is no final heuristic sanitizer. Safety comes from `DiagnosticValue`, `DiagnosticDebug`, explicit show/redact choices, private wire types, and safe typed error boundaries.

## Composition With `fastrace`

The diagnostic macro owns all span setup. Application, adapter, and infra function bodies do not import or call `Span`, `SpanContext`, `FutureExt`, `in_span`, or `current_trace_id`.

### Ordinary operations

`#[diagnostic]` inspects the ambient parent when the operation future is polled:

- when a parent exists, it creates a child span;
- when no parent exists, it creates a new random root span.

The fallback root removes the implicit requirement that a service, port implementation, adapter, or client must be called from an already instrumented caller. Direct calls remain fully logged instead of panicking or running without trace enrichment.

For inherent methods, the attribute is placed on the method. For `async_trait` production port implementations, `#[diagnostic]` remains before `#[async_trait::async_trait]`; the impl form consumes method policies before async lowering.

### Explicit roots

`#[diagnostic(root)]` always creates a new random trace, even if an ambient parent exists. It is reserved for true entry points:

- future Tauri commands;
- startup bootstrap;
- future process entry points.

### Detached tasks

`#[diagnostic(detached)]` is reserved for async detached runner entry points. The generated wrapper captures the ambient parent synchronously when the runner future is created, then returns a `Send + 'static` future that runs call logging and the original body inside a child span. When no parent exists, it captures a new random root context instead.

Detached runner methods take owned `self` and owned inputs so their returned futures can be passed directly to `tokio::spawn`:

```rust
tokio::spawn(self.clone().run_cleanup(...));
```

The caller contains no tracing setup. The macro rejects `detached` on synchronous functions and rejects combinations with `root` or `restore`.

### Stored trace restoration

`#[diagnostic(restore = operation.trace_id)]` accepts an expression of type `Option<Uuid>`:

- `Some(trace_id)` creates a new root span segment within that stored trace;
- `None` creates a new random root trace.

Only the trace ID is restored. The pre-restart span tree and parent span ID no longer exist and are not reconstructed. `restore` is reserved for per-operation interrupted recovery and is mutually exclusive with `root` and `detached`.

## Runtime Operations

`RuntimeOperation.trace_id` is `Option<Uuid>`. Operation IDs remain required `Uuid` values.

`RuntimeOperation::running` no longer accepts a trace argument. It captures the current ambient trace through the diagnostics runtime and stores it as `Some(Uuid)`. A direct constructor call outside an instrumented operation stores `None`; it does not generate an unbacked trace ID and does not panic.

SQLite stores the UUID string in a nullable `trace_id` column. A valid stored string maps to `Some(Uuid)`, `NULL` maps to `None`, and an invalid non-null string maps to `CorruptData`.

Provision and cleanup runners use `#[diagnostic(detached)]` and inherit the initiating command trace. During startup recovery, each interrupted operation is passed to a `#[diagnostic(restore = operation.trace_id)]` boundary. Recovery records therefore use the original operation trace when available without manual span code or provider reconciliation.

## Error Safety

The macro logs only the public boundary's typed error. It does not traverse `Error::source()` or serialize arbitrary dependency errors.

An error shown with `show_error` must implement `DiagnosticValue`. Error enums that need visible values derive `DiagnosticDebug` and opt in only safe fields. Lower layers map raw network, keyring, database, JSON, GraphQL, and provider failures into safe typed errors before they cross a logged public boundary.

Errors are intentionally logged at every instrumented public boundary they cross. Their shared trace ID correlates the infra, adapter, and application views of the failure.

## Initialization

`diagnostics::init(logs_dir)` configures:

- one append-only JSONL file at `<app_data_dir>/logs/luma-forge.log`;
- the standard `logforth::layout::JsonLayout`;
- application targets at `INFO` and above;
- `FastraceDiagnostic` for active `trace_id` and `span_id` fields.

Initialization occurs after support paths exist and before application state construction or operations. The current refactor shell has no Tauri setup or support-path bootstrap, so this work provides and tests the initialization API but does not invent a temporary call from the current `Hello, world!` main. The future native bootstrap must call it at the required point.

## Permanent Repository Rule

`src-tauri/AGENTS.md` records:

- which public operations require `#[diagnostic]`;
- that private helpers and test fakes are excluded;
- that detached runner and per-operation interrupted-recovery entry points are the only private exceptions;
- that external boundary DTOs use `DiagnosticDebug`;
- that logged values require explicit `show` or `redact`;
- that raw wire DTOs, request bodies, provider responses, and exposed secrets never enter diagnostic values;
- that diagnostic macros own root fallback, detached propagation, and stored-trace restoration;
- that application, adapter, and infra function bodies contain no direct tracing API calls.

## Testing

The proc-macro crate uses `trybuild` as a dev-only dependency.

Compile-pass cases cover:

- showing a whitelisted scalar;
- showing a `DiagnosticDebug` type;
- redacting an arbitrary type;
- omitted arguments and fields;
- omitted, shown, and redacted outputs;
- omitted, shown, and redacted errors;
- async inherent methods;
- `async_trait` impl instrumentation;
- root entry points;
- detached async entry points;
- stored-trace restoration from `Some(Uuid)` and `None`;
- structs and enums derived with `DiagnosticDebug`.

Compile-fail cases cover:

- showing a struct with only ordinary `Debug`;
- showing an output or error without `DiagnosticValue`;
- conflicting annotations;
- destructuring parameters;
- `#[diagnostic]` on a function not returning `Result`;
- `detached` on a synchronous function;
- conflicting `root`, `detached`, and `restore` modes;
- deriving ordinary `Debug` together with `DiagnosticDebug`.

Runtime diagnostics tests cover:

- `start -> success` ordering;
- `start -> error` ordering;
- omission of unannotated arguments, fields, outputs, and error values;
- `[REDACTED]` for redacted arguments, fields, outputs, and errors;
- the standard JSON output shape;
- `trace_id` and `span_id` enrichment;
- standalone root fallback and nested child relationships;
- detached propagation with the same trace and a distinct span;
- stored-trace restoration with `Some(Uuid)` and root fallback with `None`.

Runtime operation tests cover:

- storing the active trace as `Some(Uuid)` and storing `None` outside a span;
- detached runner propagation;
- recovery under the stored trace ID.

No instrumentation-only test is added for every application or adapter method. A final source audit verifies attribute coverage on the public operations in scope.

Native verification runs:

```sh
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Command code generation and frontend verification are required only when future Tauri command signatures or generated binding-safe types change.

## Acceptance Criteria

- Active diagnostics and its macros are implemented from scratch; no old diagnostics implementation is copied or adapted.
- Every executable public operation in scope, every detached runner entry point, and every per-operation interrupted-recovery entry point uses the diagnostic attribute.
- Ordinary calls share the entry-point trace without trace parameters in their signatures and create a fallback root when called standalone.
- The custom attribute owns ordinary root fallback, explicit roots, detached propagation, and stored-trace restoration while remaining compatible with `async_trait`.
- `DiagnosticDebug` supports structs and enums and omits unannotated fields.
- `show` accepts only `DiagnosticValue`; arbitrary ordinary `Debug` values cannot enter logs.
- Inputs, outputs, and error values are omitted unless explicitly shown or redacted.
- External boundary I/O uses application-owned DTOs with `DiagnosticDebug`; raw generated wire DTOs remain private.
- Secrets, credentials, raw bodies, raw provider responses, and large payloads never enter log values.
- Runtime operations persist the active trace as `Option<Uuid>` without requiring an ambient span, detached work preserves its parent context, and interrupted recovery restores the stored trace when present.
- Application, adapter, and infra function bodies contain no direct `fastrace` span/context propagation calls.
- `luma-forge.log` uses standard `logforth` JSON records enriched with active trace and span IDs.
- The permanent instrumentation rule is documented in `src-tauri/AGENTS.md`.
- Proc-macro compile tests and native verification pass.
