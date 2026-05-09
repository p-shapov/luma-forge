## 1. Command Error Boundary

- [x] 1.1 Move `NativeCommandError` and `NativeCommandErrorCode` out of provider setup code into a command-owned module.
- [x] 1.2 Update command result aliases and imports so Tauri command handlers own command-error mapping.
- [x] 1.3 Keep generated command error fields UI-safe and verify provider secrets cannot appear in command errors.

## 2. Provider Error Boundary

- [x] 2.1 Add a provider-local error type for provider client transport, authorization, identity, and response mapping failures.
- [x] 2.2 Change RunPod identity and inventory methods to return provider-local errors instead of use-case errors.
- [x] 2.3 Map provider-local errors to `ProviderSetupError` in the provider identity gateway implementation.
- [x] 2.4 Map provider-local errors to `WorkspaceSetupError` in the provider inventory gateway implementation.
- [x] 2.5 Update RunPod provider tests to assert provider-local mapping behavior without depending on use-case errors inside the client.

## 3. Module Layout Cleanup

- [x] 3.1 Split provider setup into a directory with separate contract, service, and test files if the command-error move leaves clear boundaries.
- [x] 3.2 Keep workspace native files under `src-tauri/src/workspace/` with `workspace_`-prefixed implementation and test files.
- [x] 3.3 Verify domain modules do not import bundled, provider, command, Tauri, storage, or provider-specific HTTP/GraphQL types.

## 4. Workspace Persistence Cleanup

- [x] 4.1 Derive the persisted workspace `gpu_cloud_provider_id` column value from `workspace.gpu_cloud_provider_id`.
- [x] 4.2 Add or update workspace catalog tests to verify the stored provider id column is consistent with the serialized workspace payload.

## 5. Verification

- [x] 5.1 Run `cargo test`.
- [x] 5.2 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 5.3 Run `cargo fmt`.
- [x] 5.4 Check whether generated TypeScript command bindings changed; if they did, run `bun run build` and `bun run lint --fix`.

## 6. Workspace Setup Module Layout

- [x] 6.1 Split workspace setup command DTOs into `workspace_setup_contracts.rs`.
- [x] 6.2 Move workspace setup service, error, gateway trait, and validation into `workspace_setup_service.rs`.
- [x] 6.3 Keep workspace setup tests in `workspace_setup_tests.rs`.
- [x] 6.4 Update imports and verify command bindings are unchanged or regenerated.

## 7. Workspace Catalog Module Layout

- [x] 7.1 Split workspace catalog repository trait and unavailable fallback into `workspace_catalog_repository.rs`.
- [x] 7.2 Move SQLite workspace catalog implementation into `workspace_catalog_sqlite.rs`.
- [x] 7.3 Keep workspace catalog tests in `workspace_catalog_tests.rs`.
- [x] 7.4 Update imports and verify behavior is unchanged.

## 8. Command Module Layout

- [x] 8.1 Move GPU cloud provider setup command handlers into `provider_setup_commands.rs`.
- [x] 8.2 Move workspace command handlers and workspace command helpers into `workspace_commands.rs`.
- [x] 8.3 Keep command builder registration in `commands/mod.rs`.
- [x] 8.4 Verify generated command bindings are unchanged or regenerated.

## 9. Bundled Catalog Module Layout

- [x] 9.1 Move the workspace setup catalog reader trait out of `/bundled` and into the workspace setup use case.
- [x] 9.2 Split bundled catalog parsing and validation into focused bundled modules.
- [x] 9.3 Keep bundled catalog tests in a separate test file.
- [x] 9.4 Verify behavior and generated command bindings are unchanged.

## 10. Workspace Setup Error Module Layout

- [x] 10.1 Move `WorkspaceSetupError` into `workspace_setup_error.rs`.
- [x] 10.2 Update workspace, provider, command, and bundled imports to use the error-owned module.
- [x] 10.3 Verify behavior and generated command bindings are unchanged.

## 11. Provider Setup Error Module Layout

- [x] 11.1 Move `ProviderSetupError` into `provider_setup_error.rs`.
- [x] 11.2 Keep provider setup public exports stable for command, provider, secret, and workspace callers.
- [x] 11.3 Verify behavior and generated command bindings are unchanged.

## 12. Bundled Naming Cleanup

- [x] 12.1 Remove the stale `bundled_catalog.rs` file after the reader split.
- [x] 12.2 Rename `bundled_contracts.rs` to `bundled_catalog_contracts.rs`.
- [x] 12.3 Verify behavior and generated command bindings are unchanged.

## 13. Provider Module Layout

- [x] 13.1 Move provider registry gateway adaptation into `provider_client_registry.rs`.
- [x] 13.2 Split RunPod implementation into client, contracts, mapper, and tests under `provider/runpod/`.
- [x] 13.3 Rename provider client errors to `provider_client_error.rs`.
- [x] 13.4 Keep provider exports explicit for command and service callers.
- [x] 13.5 Verify behavior and generated command bindings are unchanged.

## 14. Provider Secret Injection

- [x] 14.1 Inject `SecretStore` into `ProviderClientRegistry`.
- [x] 14.2 Remove explicit API key passing from workspace provider inventory gateway methods.
- [x] 14.3 Keep submitted-key validation explicit for provider setup.
- [x] 14.4 Verify behavior and generated command bindings are unchanged.

## 15. Provider Client Registry Test Layout

- [x] 15.1 Move provider client registry tests into `provider_client_tests.rs`.
- [x] 15.2 Verify behavior and generated command bindings are unchanged.

## 16. Command Infrastructure Module Layout

- [x] 16.1 Move command registration into `command_builder.rs`.
- [x] 16.2 Rename `bindings.rs` to `command_bindings.rs`.
- [x] 16.3 Keep command module exports stable for app startup.
- [x] 16.4 Verify behavior and generated command bindings are unchanged.

## 17. Command Handler Directory Layout

- [x] 17.1 Move provider setup command handlers into `commands/provider_setup/`.
- [x] 17.2 Move workspace command handlers into `commands/workspace/`.
- [x] 17.3 Keep command builder registration readable through nested handler module paths.
- [x] 17.4 Verify behavior and generated command bindings are unchanged.
