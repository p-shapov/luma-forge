## Context

LumaForge already treats the Native Layer as authoritative for persisted state, provider access, secure storage, and long-running operations. Command errors returned to React are intentionally UI-safe and do not expose provider secrets, keyring details, or raw provider transport data.

The native crate currently has `tracing` and `tracing-subscriber` dependencies, but no application-wide tracing setup and only minimal direct tracing usage. This change moves native diagnostics toward the official Tauri logging plugin and the Rust `log` facade, with logging concentrated at Tauri command boundaries.

The first logging use case is local debugging of native command execution: command start, command outcome, latency, and safe error classification. The design should not turn application services into logging emitters, and it should not introduce frontend logging yet.

## Goals / Non-Goals

**Goals:**

- Add durable native-side logs for Tauri command execution.
- Use the official Tauri logging plugin as the native log sink.
- Keep logging concentrated at the command boundary for the initial implementation.
- Include an operation identifier in command lifecycle records so related start and finish/failure events can be correlated.
- Log only UI-safe command metadata and UI-safe error metadata.
- Remove the unused `tracing`/`tracing-subscriber` direction for this phase.
- Preserve service and domain independence from Tauri runtime APIs and generated command concerns.

**Non-Goals:**

- Do not add frontend/client logging.
- Do not add remote telemetry, crash reporting, Sentry, OpenTelemetry, or analytics.
- Do not add a persistent diagnostic journal in SQLite.
- Do not log raw command payloads or provider request/response bodies.
- Do not instrument every application service, repository, validator, mapper, or provider helper.
- Do not change generated command bindings or command response shapes.

## Decisions

### Use the official Tauri log plugin as the sink

The native app will initialize `tauri-plugin-log` during Tauri builder setup. The plugin provides standard app log targets and leaves a future path for frontend logging through the same plugin if the product later needs it.

Alternative considered: initialize `tracing_subscriber` directly. That would support spans and richer structured tracing, but the current requirement is simpler native command logs, and mixing `tracing` with Tauri plugin logging would add complexity before there is a concrete need.

Alternative considered: hand-roll file logging. That would duplicate behavior already provided by the official plugin and create unnecessary decisions around log paths, rotation, and platform behavior.

### Use `log` macros at command boundaries

Command handlers will log command start and command completion/failure with the Rust `log` facade. This keeps logging close to the Tauri adapter layer, where command names, safe command DTO context, elapsed time, and mapped command errors are available.

The command boundary may log:

- command name
- generated operation identifier
- provider id when present and UI-safe
- workspace id when present and UI-safe
- status such as `ok` or `error`
- elapsed time
- native command error code, retryability, reason, field, and recovery action

The command boundary must not log raw command payloads. The `setup_gpu_cloud_provider` command in particular must not log the submitted Provider API Key.

Alternative considered: log directly inside every service method. That would provide more internal detail, but it spreads operational concerns through application services and makes secret review harder.

### Keep services silent by default

Application services should remain free of direct logging calls for this change. This preserves the existing architecture where services coordinate domain work and side effects, while Tauri commands own generated command contracts and adapter behavior.

If a later multi-phase workflow needs internal diagnostics, the service should receive a small Tauri-independent observer or diagnostics trait through dependency injection. The observer should accept typed, safe diagnostic events rather than arbitrary log strings. A production implementation can write those events to `log`, while tests can record them.

Alternative considered: pass a Tauri logger into services. This would couple application logic to the Tauri runtime and violate the Native Layer separation already used for command DTOs, provider clients, and secret storage.

### Remove `tracing` for this phase

The implementation should remove direct `tracing` usage and remove `tracing`/`tracing-subscriber` dependencies unless another necessary dependency path requires them. Existing minimal tracing usage should be converted to `log` with the same secret-safe content.

Alternative considered: keep `tracing` in case it becomes useful later. Keeping unused diagnostic infrastructure makes the logging model ambiguous and increases the chance that future code mixes two logging styles.

### Treat logs as durable diagnostics

Because app logs can persist on disk and may later be shared for support, log content must follow the same safety posture as command responses and diagnostics. Logs are not a place for raw source errors when those errors may include secrets, credentials, provider transport details, or implementation internals that are unsafe to expose.

## Risks / Trade-offs

- Command-level logs may be too coarse for future provisioning failures -> Add a narrow DI observer for specific multi-phase workflows when a concrete debugging gap appears.
- Developers may accidentally log raw request DTOs -> Make the spec explicitly forbid raw command payload logging and add targeted tests around provider API key submission.
- Removing `tracing` loses span-style context -> Accept for this phase because operation identifiers and command lifecycle events cover the current need.
- Tauri plugin defaults may log more broadly than intended -> Configure plugin level and targets intentionally, and avoid enabling frontend log permissions or client usage in this change.
- Logs become another durable surface for sensitive data -> Keep allowed and forbidden fields explicit, and review log call sites as part of implementation.

## Migration Plan

1. Add `tauri-plugin-log` and `log` to native dependencies.
2. Initialize the Tauri log plugin in the app builder.
3. Replace existing `tracing::warn!` usage with an equivalent safe `log` call.
4. Remove `tracing` and `tracing-subscriber` dependencies if no remaining direct usage exists.
5. Add command boundary logging helpers or wrappers that generate an operation id, measure elapsed time, and log safe command outcomes.
6. Instrument existing Tauri commands through the command boundary logging path.
7. Verify native tests, clippy, and formatting.

Rollback is straightforward: remove command logging call sites and plugin initialization, then remove the logging plugin dependency. Because no command contract or persistence schema changes are expected, rollback does not require data migration.

## Open Questions

- What default log level should release builds use: `info` for command lifecycle or `warn` for failures only?
- Should command start events be logged in release builds, or only command completion/failure events?
- Should local log rotation use plugin defaults or an explicit size limit?
