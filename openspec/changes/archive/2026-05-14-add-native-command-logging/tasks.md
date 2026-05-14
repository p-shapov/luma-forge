## 1. Dependencies and Plugin Setup

- [x] 1.1 Add native dependencies for `tauri-plugin-log` and `log`.
- [x] 1.2 Remove direct `tracing` and `tracing-subscriber` dependencies if no remaining direct usage requires them.
- [x] 1.3 Initialize the official Tauri logging plugin in the native app builder with intentional native log targets and levels.
- [x] 1.4 Keep frontend logging unconfigured; do not add `@tauri-apps/plugin-log` or frontend log permissions unless required by the native plugin setup.

## 2. Command Boundary Logging

- [x] 2.1 Add a command logging helper or wrapper that creates an operation id, records elapsed time, and emits command start logs.
- [x] 2.2 Extend the command logging helper or wrapper to emit command success logs with command name, operation id, outcome, and elapsed time.
- [x] 2.3 Extend the command logging helper or wrapper to emit command failure logs with command name, operation id, outcome, elapsed time, and UI-safe native command error metadata.
- [x] 2.4 Instrument existing Provider Setup commands through the command boundary logging path without logging raw request payloads.
- [x] 2.5 Instrument existing Workspace Setup commands through the command boundary logging path without changing command response shapes or generated bindings.

## 3. Secret-Safe Migration

- [x] 3.1 Replace existing direct `tracing::warn!` usage with equivalent `log` usage that preserves only UI-safe diagnostic metadata.
- [x] 3.2 Audit command logging call sites to ensure Provider API Keys, bearer headers, raw provider request bodies, raw provider response bodies, raw keyring diagnostics, and raw command payloads are not logged.
- [x] 3.3 Ensure application services, domain modules, validators, mappers, and provider clients do not gain direct Tauri logging dependencies as part of this change.

## 4. Verification

- [x] 4.1 Add or update native tests for command logging metadata where practical, especially that provider setup logging does not include the submitted Provider API Key.
- [x] 4.2 Run `cargo test`.
- [x] 4.3 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 4.4 Run `cargo fmt`.
