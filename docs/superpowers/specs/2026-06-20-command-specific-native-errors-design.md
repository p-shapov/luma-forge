# Command-Specific Native Errors Design

## Purpose

`NativeCommandErrorCode` currently aggregates error codes for unrelated native commands. That makes every command appear able to fail with system-wide variants, even when a command has a much narrower failure surface.

Refactor native command errors so each Tauri command exposes its own error code enum and its own command-specific error mapping.

## Scope

In scope:

- Replace the ordinary command result contract with a generic command error envelope whose `code` is command-specific.
- Add a distinct public Specta error code enum for each Tauri command.
- Move domain-to-command error mapping from global `NativeCommandErrorCode` conversions to per-command mappers.
- Regenerate TypeScript command bindings after native contract changes.
- Keep diagnostics, redaction, diagnostic IDs, and structured logging behavior.

Out of scope:

- Legacy compatibility for the current global `NativeCommandErrorCode`.
- New command-layer unit tests.
- Hand edits to `src/generated/commands.ts`.
- Frontend behavior changes beyond compile fixes caused by generated type changes.

## Contract

Use one shared envelope shape for command failures:

```rust
CommandError<Code> {
    message,
    code,
    diagnostic_id,
}
```

Each command returns a result whose error code type is specific to that command, for example:

- `SetupRunpodApiKeyErrorCode`
- `GetRunpodApiKeyIdentityErrorCode`
- `DeleteRunpodApiKeyErrorCode`
- `GetWorkflowCatalogErrorCode`
- `GetRuntimeContractCatalogErrorCode`
- `GetRunpodPlacementOptionsErrorCode`
- `GetWorkspaceCatalogErrorCode`
- `CreateRunpodWorkspaceErrorCode`
- `ProvisionWorkspaceErrorCode`
- `CleanupWorkspaceErrorCode`
- `DeleteWorkspaceErrorCode`
- `GetRunningLifecycleOperationsErrorCode`
- `GetLatestLifecycleOperationErrorCode`

The generated TypeScript bindings should expose a specific error type per command through `typedError<Success, CommandError<SpecificCode>>` or the equivalent generated representation.

Startup/native initialization failure remains separate from ordinary command errors. It can keep its dedicated startup status error contract because it represents native bootstrap state, not a normal command failure surface.

## Mapping

Remove the global ordinary-command requirement that any domain error can map into one aggregated `NativeCommandErrorCode`.

Each command maps only the errors it can actually return:

- Secret setup commands map secret parsing, secret storage, and identity validation failures.
- Secret identity commands map key absence, store failures, and identity request/response failures.
- Secret delete commands map delete-related key/store failures.
- Catalog commands map only their catalog parse and validation failures.
- Runpod placement options maps provider request/auth/rate-limit/timeout failures and any secret access failures that are actually reachable.
- Workspace lifecycle commands map workspace, lifecycle journal, provider, and catalog failures reachable through the called service method.

Mappings should be exhaustive for the domain error type used by the command. If a new domain error becomes reachable, compilation should force the command mapper to classify it.

Do not add silent fallback variants for impossible or deprecated behavior.

## Diagnostics

Diagnostics remain responsible for:

- Creating `diagnosticId` values.
- Redacting log messages.
- Extracting leaf error messages.
- Logging command name, request metadata, duration when available, code, error message, and source-chain context.

`command_error` should become generic over the command-specific code enum and accept an explicit mapper. It should not know the full set of all command error codes.

The logged `code` value should be the concrete command-specific enum variant.

If `source_chain` cannot preserve typed global code information without reintroducing the aggregate enum, it should become diagnostic text rather than a second typed command-code mapping path.

## Implementation Notes

Expected touched areas:

- `src-tauri/src/commands/errors.rs`
- `src-tauri/src/diagnostics/mod.rs`
- `src-tauri/src/commands/catalog.rs`
- `src-tauri/src/commands/secrets.rs`
- `src-tauri/src/commands/workspaces.rs`
- `src-tauri/src/commands/native.rs` and startup status types only if required by the type split
- generated command bindings via `bun run codegen:commands`
- frontend compile fixes only where generated types require them

Keep Tauri command handlers as adapters: they should call application services and use command-local error mapping at the boundary.

## Verification

Run native verification:

```sh
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Because command contracts change, also run:

```sh
bun run codegen:commands
bun run build
bun run lint
```

Do not add new command-layer unit tests for this refactor.
