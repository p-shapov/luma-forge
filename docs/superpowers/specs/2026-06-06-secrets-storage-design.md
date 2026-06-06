# Secrets Storage Design

## Problem Statement

LumaForge needs a native Rust `secrets_storage` service for provider API keys. The service must store raw API keys only in native secure storage, validate keys through provider APIs before writing, reject accidental overwrites, and expose a small backend-only contract for identity lookup, raw retrieval, and removal.

This spec covers only the native backend module and its tests. Tauri command wiring, generated frontend bindings, and React integration are out of scope.

## Goals

- Add a focused `src-tauri/src/secrets_storage/` module.
- Store Runpod and Hugging Face API keys through a typed key contract.
- Keep raw secrets out of UI-safe types, logs, generated bindings, snapshots, fixtures, and command responses.
- Validate provider identity before writing a new secret.
- Reject overwrite attempts; replacement requires `remove` followed by `write`.
- Return provider identity as UI-safe `ApiKeyIdentity`.
- Let backend services retrieve raw secrets when they need to call provider APIs.

## Non-Goals

- No Tauri commands for this service in this pass.
- No frontend changes or generated TypeScript updates.
- No compatibility shim for legacy setup services.
- No live provider integration tests.
- No generalized dynamic secret framework.

## Module Layout

Create:

```text
src-tauri/src/secrets_storage/
  mod.rs
  store.rs
  identity.rs
  service.rs
  errors.rs
  keyring_store.rs
  hugging_face_identity.rs
  runpod_identity.rs
```

`mod.rs` exports the public service, traits, key type, secret type, and errors that other native backend modules need.

`store.rs` owns the storage trait. It does not know provider API details.

`identity.rs` owns the provider identity trait. It does not know keyring details.

`service.rs` owns orchestration rules: existence checks, validation before write, and missing-key behavior.

`errors.rs` owns the stable error enum for the module.

`keyring_store.rs` implements secure local storage.

`hugging_face_identity.rs` and `runpod_identity.rs` implement provider API validation and response mapping.

Register the module from `src-tauri/src/lib.rs`:

```rust
pub mod secrets_storage;
```

## Domain Contract

Update `src-tauri/src/domain/secrets.rs` so `ApiKeyIdentity.email` is optional:

```rust
pub struct ApiKeyIdentity {
    pub email: Option<String>,
    pub username: Option<String>,
    pub key_display_name: Option<String>,
}
```

Runpod can populate `email`. Hugging Face may not always return an email, so the common identity contract must represent that honestly.

## Secret Key Contract

Use a typed enum instead of arbitrary strings:

```rust
pub enum SecretKey {
    RunpodApiKey,
    HuggingFaceApiKey,
}
```

The enum keeps keyring account names fixed by code and prevents callers from creating arbitrary secure-storage entries through this service.

## Secret Value Contract

Use `secrecy` for raw API keys:

```rust
pub struct ApiSecret(SecretString);
```

`ApiSecret` must reject blank values. It must not implement `Serialize` or expose raw values through `Debug`. Raw access should require an explicit method used only by storage and provider-call paths.

Using a named wrapper keeps service signatures readable and reduces accidental treatment of raw keys as ordinary strings.

## Store Trait

`store.rs` defines an async trait using the existing `AppFuture` pattern:

```rust
pub trait SecretStore: Send + Sync {
    fn has<'a>(
        &'a self,
        key: SecretKey,
    ) -> AppFuture<'a, Result<bool, SecretsStorageError>>;

    fn write<'a>(
        &'a self,
        key: SecretKey,
        secret: ApiSecret,
    ) -> AppFuture<'a, Result<(), SecretsStorageError>>;

    fn delete<'a>(
        &'a self,
        key: SecretKey,
    ) -> AppFuture<'a, Result<(), SecretsStorageError>>;

    fn read<'a>(
        &'a self,
        key: SecretKey,
    ) -> AppFuture<'a, Result<Option<ApiSecret>, SecretsStorageError>>;
}
```

The store trait stores and returns raw secrets only inside the native backend. It does not validate provider credentials.

## Identity Trait

`identity.rs` defines provider validation:

```rust
pub trait ApiKeyIdentityProvider: Send + Sync {
    fn identity<'a>(
        &'a self,
        key: SecretKey,
        secret: &'a ApiSecret,
    ) -> AppFuture<'a, Result<ApiKeyIdentity, SecretsStorageError>>;
}
```

The provider implementation must validate that the secret is currently accepted by the provider API and map the response into UI-safe identity fields.

Use one composite identity provider that matches `SecretKey` internally and delegates to the provider-specific adapters. This keeps `service.rs` independent from concrete providers without adding a generalized dynamic registry.

## Service Contract

`service.rs` defines:

```rust
pub struct SecretsStorageService<S, I> {
    store: S,
    identity: I,
}
```

Public methods:

```rust
impl<S, I> SecretsStorageService<S, I>
where
    S: SecretStore,
    I: ApiKeyIdentityProvider,
{
    pub async fn write(
        &self,
        key: SecretKey,
        secret: ApiSecret,
    ) -> Result<ApiKeyIdentity, SecretsStorageError>;

    pub async fn identity(
        &self,
        key: SecretKey,
    ) -> Result<ApiKeyIdentity, SecretsStorageError>;

    pub async fn retrieve(
        &self,
        key: SecretKey,
    ) -> Result<ApiSecret, SecretsStorageError>;

    pub async fn remove(
        &self,
        key: SecretKey,
    ) -> Result<(), SecretsStorageError>;
}
```

`write` behavior:

1. Call `store.has(key)`.
2. If true, return `KeyAlreadyExists`.
3. Validate `secret` through `identity.identity(key, &secret)`.
4. If validation fails, do not write.
5. Write the secret.
6. Return the validated `ApiKeyIdentity`.

`identity` behavior:

1. Read the stored secret.
2. If missing, return `KeyNotFound`.
3. Validate through the provider identity trait.
4. Return `ApiKeyIdentity`.

`retrieve` behavior:

1. Read the stored secret.
2. If missing, return `KeyNotFound`.
3. Return `ApiSecret`.

`retrieve` is a backend-only native method. It must not be exposed through Tauri command handlers.

`remove` behavior:

1. Call `store.has(key)`.
2. If false, return `KeyNotFound`.
3. Delete the secret.

## Errors

`errors.rs` defines:

```rust
pub enum SecretsStorageError {
    SecretRequired,
    KeyAlreadyExists,
    KeyNotFound,
    StoreUnavailable,
    StoredSecretInvalid,
    Unauthorized,
    InsufficientPermissions,
    RateLimited,
    ProviderUnavailable,
    IdentityResponseInvalid,
}
```

Error mapping rules:

- Blank submitted secret maps to `SecretRequired`.
- Existing key during `write` maps to `KeyAlreadyExists`.
- Missing key during `identity`, `retrieve`, or `remove` maps to `KeyNotFound`.
- Keyring construction, read, write, or delete failure maps to `StoreUnavailable`.
- Stored blank or otherwise invalid key material maps to `StoredSecretInvalid`.
- Provider 401 or 403 maps to `Unauthorized`, except when the provider response clearly means insufficient token scope.
- Provider rate limiting maps to `RateLimited`.
- Provider timeout, network failure, or server failure maps to `ProviderUnavailable`.
- Malformed or semantically invalid identity responses map to `IdentityResponseInvalid`.

Provider adapters must not include raw secrets in error values.

## Keyring Store

`keyring_store.rs` implements `SecretStore` using fixed service/account names derived from `SecretKey`.

Recommended account mapping:

- `RunpodApiKey` -> `runpod`
- `HuggingFaceApiKey` -> `hugging-face`

The service name should include the app identifier and a secrets-storage scope, following the legacy keyring convention without preserving legacy APIs.

Keyring operations are blocking. The async implementation should run them off the async executor thread, using the smallest local helper that matches the existing backend style.

Deleting a missing credential at the store level may return success or a store-level no-entry result, but `SecretsStorageService::remove` must still return `KeyNotFound` for missing keys because it checks `has` first.

## Runpod Identity

`runpod_identity.rs` validates `SecretKey::RunpodApiKey`.

It should call Runpod's identity GraphQL query and map a valid response into:

- `email: Some(email)` when present and non-blank
- `username: None` unless Runpod returns a stable username field
- `key_display_name: Some(...)` only if the API response provides a safe display name or fingerprint

The response is invalid when required provider identity fields are missing, blank, or not associated with an active API key.

The composite identity provider must dispatch `SecretKey::RunpodApiKey` to this adapter, so the adapter does not need to support non-Runpod keys.

## Hugging Face Identity

`hugging_face_identity.rs` validates `SecretKey::HuggingFaceApiKey`.

It should call Hugging Face `whoami-v2` and map a valid response into:

- `email: response.email`
- `username: Some(response.name)` when non-blank
- `key_display_name: Some(access token display name)` when non-blank

It should validate token permissions for model downloads if the response exposes enough permission data. A token is acceptable when it is a broad read/write token or a fine-grained token with the required read permissions for model content and gated repositories.

If the response cannot prove that the token can read required model assets, return `InsufficientPermissions`.

## Dependencies

Expected `src-tauri/Cargo.toml` additions:

- `secrecy` for raw secret handling.
- `keyring` for secure local storage.
- `reqwest` for provider identity HTTP calls.
- `thiserror` only if implementation uses displayable error enums consistently.

Use existing dependencies where already available. Do not add a dependency unless it is needed by the implementation.

## Tests

Service unit tests should use fake stores and fake identity providers.

Required service cases:

- `write` returns `KeyAlreadyExists` and does not validate or write when the key exists.
- `write` validates before writing when the key is missing.
- `write` does not write when validation fails.
- `write` returns the validated identity after successful storage.
- `identity` returns `KeyNotFound` when no secret exists.
- `identity` validates a stored secret before returning identity.
- `retrieve` returns a stored `ApiSecret`.
- `retrieve` returns `KeyNotFound` when missing.
- `remove` deletes an existing secret.
- `remove` returns `KeyNotFound` when missing.

Provider mapping tests should use static JSON payloads and no live network calls.

Runpod tests should cover:

- valid identity response
- unauthorized status mapping
- malformed or incomplete response

Hugging Face tests should cover:

- valid broad read token
- valid fine-grained token with required permissions
- missing email accepted
- insufficient permissions rejected
- unauthorized status mapping
- malformed or incomplete response

Keyring adapter tests should avoid relying on the user's OS keychain. Prefer a fake credential backend if the selected crate supports it. If that creates too much test scaffolding, keep keyring adapter tests narrow and put most behavior coverage at the service level.

## Verification

For this native backend change, run from the repository root:

```text
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Because command contracts are out of scope, `bun run codegen:commands`, `bun run build`, and `bun run lint` are not required for this spec.

## Security Notes

- `ApiSecret` must not implement serialization.
- `Debug` for secret-bearing values must be redacted.
- Provider adapters must not log request headers, bearer tokens, or raw response bodies that might echo credentials.
- Test fixtures must not contain real credentials.
- `retrieve` must remain native-only and must not be exposed to React or generated bindings.

## Open Implementation Checks

- Confirm the current Hugging Face `whoami-v2` response still exposes enough permission data to enforce download access. If it does not, fail closed with `InsufficientPermissions` and document the exact limitation in code comments near the mapper.
- Confirm the selected `keyring` crate version supports a fake credential backend for tests. If not, prefer service-level coverage over OS-keychain-dependent tests.
