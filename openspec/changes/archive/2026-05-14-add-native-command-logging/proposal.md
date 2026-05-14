## Why

Native failures currently return UI-safe command errors, but there is no durable native-side log trail for debugging command execution, latency, or failure outcomes. As provider setup, workspace setup, and later provisioning flows grow more operationally complex, LumaForge needs local native logs that help debug behavior without exposing secrets or spreading logging concerns through application services.

## What Changes

- Add native-only command boundary logging for Tauri commands.
- Use the official Tauri logging plugin as the local log sink so native logs are written to supported app log targets and can later be extended to frontend logging if needed.
- Replace the unused `tracing`/`tracing-subscriber` dependency direction with the simpler `log` facade and Tauri log plugin.
- Emit safe command lifecycle records including command name, operation identifier, outcome, elapsed time, and safe native error metadata.
- Keep service-layer logging out of scope by default; service internals may only expose future diagnostic events through a Tauri-independent DI observer if a multi-phase workflow needs it.
- Preserve the existing secret boundary: logs must not include provider API keys, stored secrets, raw command payloads, keyring internals, raw provider transport data, raw provider response bodies, or other unsafe diagnostics.

## Capabilities

### New Capabilities

- `native-command-logging`: Defines native command logging behavior, allowed metadata, forbidden sensitive values, log sink expectations, and service-layer boundaries.

### Modified Capabilities

- None.

## Impact

- Affected native code: Tauri app bootstrap, command handlers, command error mapping call sites, and native dependency configuration.
- Affected dependencies: add the official Tauri log plugin and `log`; remove `tracing` and `tracing-subscriber` unless another active implementation dependency requires them.
- Affected frontend code: none for this change. Frontend logging through the Tauri plugin remains a future extension.
- Affected generated command bindings: none expected.
- Affected security posture: logs become a new durable diagnostic surface and must follow the same secret isolation rules as command responses and diagnostics.
