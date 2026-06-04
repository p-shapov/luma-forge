# Provider API Key Module Design

## Context

LumaForge stores GPU provider credentials behind native secure storage and uses those credentials only through trusted backend paths. The active native backend in `src-tauri/src` is a minimal refactor shell. Legacy provider setup code exists in `src-tauri-legacy`, but it is reference material only; the active backend should get a narrow current-contract module instead of restoring the legacy workflow wholesale.

The immediate need is a backend module named `provider_api_key` with methods for retrieving a stored provider API key and deriving provider setup status from that stored key. This iteration does not expose new Tauri commands and does not add production keyring or RunPod HTTP adapters.

## Goals

- Add an active-backend `provider_api_key` module under `src-tauri/src`.
- Provide service methods:
  - `ProviderApiKeyService::has_key`
  - `ProviderApiKeyService::read_key`
  - `ProviderApiKeyService::write_key`
  - `ProviderApiKeyService::remove_key`
  - `ProviderApiKeyService::validate_identity`
- Keep raw provider API keys backend-only and redacted from debug output.
- Derive provider setup by validating provider identity with the stored key.
- Define narrow storage and provider-identity traits with fake implementations in tests.
- Keep the module ready for later production keyring and RunPod adapter wiring.

## Non-Goals

- No Tauri command wiring.
- No generated frontend binding changes.
- No real keyring implementation.
- No real RunPod HTTP implementation.
- No Tauri setup/delete command workflow.
- No compatibility layer for legacy provider setup contracts.
- No fallback to another provider or old storage location.

## Module Shape

Add:

```text
src-tauri/src/provider_api_key/
  mod.rs
  error.rs
  service.rs
  store.rs
  provider.rs
```

Expose it from `src-tauri/src/lib.rs`:

```rust
pub mod provider_api_key;
```

The module is an application/service boundary. It owns key retrieval and provider setup discovery, but delegates actual storage and provider identity validation to injected collaborators.

## Domain Types

The active backend already has:

- `domain::provider::GpuCloudProviderId` for the provider identifier.
- `domain::shared::ApiKeySetup` for UI-safe API key setup/identity data.

Reuse both types. Do not introduce provider-specific setup or identity structs for this iteration.

Add a backend-only `ProviderApiKey` secret wrapper in the `provider_api_key` module unless a broader active-domain secret wrapper is introduced first.

`ProviderApiKey`:

- rejects blank values;
- can expose the raw secret only through an explicit method for trusted provider-call paths;
- implements `Debug` without printing the raw key;
- is not serialized, exported to Specta, or returned from commands.

Use `ApiKeySetup` as the setup snapshot and provider identity value:

```rust
pub struct ApiKeySetup {
    pub email: String,
    pub username: String,
    pub key_display_name: String,
}
```

All `ApiKeySetup` fields must be non-blank and must not contain control characters before they are accepted as setup status.

## Collaborator Traits

Storage trait:

```rust
pub trait ProviderApiKeyStore: Send + Sync {
    fn has_key<'a>(&'a self, provider_id: GpuCloudProviderId) -> AppFuture<'a, Result<bool, ProviderApiKeyError>>;

    fn read_key<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
    ) -> AppFuture<'a, Result<Option<ProviderApiKey>, ProviderApiKeyError>>;

    fn write_key<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        api_key: &'a ProviderApiKey,
    ) -> AppFuture<'a, Result<(), ProviderApiKeyError>>;

    fn remove_key<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
    ) -> AppFuture<'a, Result<(), ProviderApiKeyError>>;
}
```

Provider identity trait:

```rust
pub trait ProviderIdentityValidator: Send + Sync {
    fn validate_identity<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        api_key: &'a ProviderApiKey,
    ) -> AppFuture<'a, Result<ApiKeySetup, ProviderApiKeyError>>;
}
```

Use the shared boxed future alias in trait methods to keep the traits object-safe without adding a new dependency:

```rust
use crate::shared::AppFuture;
```

`AppFuture` is defined in `src-tauri/src/shared.rs`.

## Service Behavior

`ProviderApiKeyService` owns the two collaborators:

```rust
pub struct ProviderApiKeyService<S, V> {
    store: S,
    validator: V,
}
```

`has_key(provider_id)`:

- calls storage only;
- returns whether a key entry exists;
- maps storage failures to a typed UI-safe error;
- never validates provider identity.

`read_key(provider_id)`:

- reads the key from storage;
- returns `ProviderApiKey` when present;
- returns `ProviderSetupIncomplete` when missing;
- never logs, serializes, or formats the raw key;
- does not call the provider validator.

`write_key(provider_id, api_key)`:

- accepts only a constructed `ProviderApiKey`, so raw key validation happens before storage is touched;
- returns `ProviderSetupAlreadyExists` when a stored key already exists for the provider;
- rejects an existing stored key before provider identity validation;
- validates provider identity with the submitted key before writing storage;
- validates returned `ApiKeySetup` fields before writing;
- writes the stored key for the provider only after identity validation succeeds;
- returns the validated `ApiKeySetup` after a successful write;
- maps provider validation failures without mutating storage;
- maps storage check failures to `SecureKeyringUnavailable`;
- maps storage write failures to `SecureKeyringUnavailable`;
- never returns the raw key.

`remove_key(provider_id)`:

- removes the stored key for the provider;
- returns `ProviderSetupIncomplete` when no key exists;
- maps storage remove/check failures to `SecureKeyringUnavailable`;
- never validates provider identity;
- never returns the raw key.

`validate_identity(provider_id)`:

1. Calls `read_key(provider_id)`.
2. Calls `validator.validate_identity(provider_id, &api_key)`.
3. Validates returned `ApiKeySetup` fields.
4. Returns `ApiKeySetup`.

This method treats provider setup as complete only when a stored key exists and the current provider identity validation succeeds. `write_key` uses the same provider identity validation rules before persisting a submitted key, so an unauthorized or malformed provider key is not stored.

## Errors

Define one narrow error enum for this module:

```rust
pub enum ProviderApiKeyError {
    ProviderSetupIncomplete,
    ProviderSetupAlreadyExists,
    StoredProviderApiKeyInvalid,
    SecureKeyringUnavailable,
    ProviderUnauthorized,
    ProviderRateLimited,
    ProviderTimeout,
    ProviderRequestFailed { message: String },
    ProviderIdentityResponseInvalid,
}
```

Provider API failures should follow the same flattened style as `remote_workspace::errors::RemoteWorkspaceError`: use explicit provider variants on the module error enum instead of a nested provider error type. Error values must be UI-safe. They must not include raw keys, raw provider responses, request bodies, headers, bearer tokens, or SDK debug output.

Expected mapping:

- missing storage entry -> `ProviderSetupIncomplete`;
- existing storage entry during `write_key` -> `ProviderSetupAlreadyExists`;
- blank/invalid stored key -> `StoredProviderApiKeyInvalid`;
- storage read/write/remove/check failure -> `SecureKeyringUnavailable`;
- provider unauthorized -> `ProviderUnauthorized`;
- provider rate limit -> `ProviderRateLimited`;
- provider timeout -> `ProviderTimeout`;
- other provider/network failure -> `ProviderRequestFailed { message }` with a UI-safe message;
- provider identity response is structurally invalid or has blank/control-character `ApiKeySetup` fields -> `ProviderIdentityResponseInvalid`.

## Security Constraints

- Raw provider API keys stay behind `ProviderApiKey`.
- `ProviderApiKey` must not implement `Serialize`, `Deserialize`, or Specta `Type`.
- `Debug` for `ProviderApiKey` must redact the secret.
- Tests must not assert on leaked raw secrets through formatted output.
- Setup snapshots may include only the UI-safe `ApiKeySetup` fields: email, username, and key display name.
- No command response or generated frontend type may include the raw key in this iteration or later wiring.

## Testing

Add focused unit tests under the new module with fake store and fake validator implementations.

Required scenarios:

- `has_key` returns `true` and `false` from storage.
- `has_key` maps storage failure to `SecureKeyringUnavailable`.
- `read_key` returns a stored key.
- `read_key` returns `ProviderSetupIncomplete` when no key exists.
- `read_key` maps invalid stored key to `StoredProviderApiKeyInvalid`.
- `write_key` returns `ProviderSetupAlreadyExists` without provider validation when a key already exists.
- `write_key` validates provider identity before writing storage.
- `write_key` stores a non-blank `ProviderApiKey` only after provider identity validation succeeds.
- `write_key` returns the validated `ApiKeySetup` after storing.
- blank raw key input cannot be written because `ProviderApiKey` construction rejects it before `write_key`.
- `write_key` maps storage check failure to `SecureKeyringUnavailable` without provider validation.
- `write_key` maps storage write failure to `SecureKeyringUnavailable`.
- `write_key` maps unauthorized provider validation to `ProviderUnauthorized` without mutating storage.
- `write_key` maps rate-limited provider validation to `ProviderRateLimited` without mutating storage.
- `write_key` maps provider timeout to `ProviderTimeout` without mutating storage.
- `write_key` maps other UI-safe provider failures to `ProviderRequestFailed { message }` without mutating storage.
- `write_key` rejects blank or control-character `ApiKeySetup` fields as `ProviderIdentityResponseInvalid` without mutating storage.
- `remove_key` removes an existing provider API key.
- `remove_key` returns `ProviderSetupIncomplete` when no key exists.
- `remove_key` maps storage remove/check failure to `SecureKeyringUnavailable`.
- `remove_key` does not call provider identity validation.
- `ProviderApiKey` rejects blank values.
- `ProviderApiKey` debug output is redacted.
- `validate_identity` validates identity with the stored key.
- `validate_identity` returns `ApiKeySetup` with email, username, and key display name.
- `validate_identity` returns `ProviderSetupIncomplete` when the key is missing.
- `validate_identity` maps unauthorized provider validation to `ProviderUnauthorized`.
- `validate_identity` maps rate-limited provider validation to `ProviderRateLimited`.
- `validate_identity` maps provider timeout to `ProviderTimeout`.
- `validate_identity` maps other UI-safe provider failures to `ProviderRequestFailed { message }`.
- `validate_identity` rejects blank or control-character `ApiKeySetup` fields as `ProviderIdentityResponseInvalid`.

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

No `bun run codegen:commands` is required because this spec does not change Tauri command contracts.

## Future Work

Later iterations may add:

- a keyring-backed `ProviderApiKeyStore`;
- a RunPod-backed `ProviderIdentityValidator`;
- Tauri command wiring for provider API key setup/status;
- command flow for setup reset when UI/API reset behavior is in scope.

Those additions should use the traits from this module instead of changing the service contract.
