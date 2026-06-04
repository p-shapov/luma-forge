# Provider API Key Module Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an active-backend `provider_api_key` module that stores provider API keys behind injected storage, validates provider identity through an injected collaborator, and never exposes raw keys outside trusted backend paths.

**Architecture:** Add `src-tauri/src/provider_api_key/` as an application/service module. `ProviderApiKeyService` owns setup discovery and key lifecycle behavior, while object-safe store and provider identity traits isolate secure storage and provider API calls for later production adapters.

**Tech Stack:** Rust 2021, Tauri backend crate, `crate::shared::AppFuture`, existing domain types, native `cargo test/fmt/clippy`.

---

## File Structure

- Create `src-tauri/src/provider_api_key/mod.rs`: module exports.
- Create `src-tauri/src/provider_api_key/error.rs`: UI-safe provider API key errors.
- Create `src-tauri/src/provider_api_key/store.rs`: key secret wrapper and storage trait.
- Create `src-tauri/src/provider_api_key/provider.rs`: provider identity validator trait.
- Create `src-tauri/src/provider_api_key/service.rs`: service implementation and focused unit tests.
- Modify `src-tauri/src/lib.rs`: expose `pub mod provider_api_key;`.

Do not add Tauri commands, generated frontend bindings, production keyring code, or a real RunPod HTTP adapter in this plan.

## Task 1: Module Shell And Error Boundary

**Files:**
- Create: `src-tauri/src/provider_api_key/mod.rs`
- Create: `src-tauri/src/provider_api_key/error.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add the module shell**

Create `src-tauri/src/provider_api_key/mod.rs`:

```rust
pub mod error;
pub mod provider;
pub mod service;
pub mod store;
```

Modify `src-tauri/src/lib.rs` near the other backend modules:

```rust
pub mod domain;
pub mod provider_api_key;
pub mod remote_workspace;
pub mod shared;
```

- [ ] **Step 2: Add UI-safe errors**

Create `src-tauri/src/provider_api_key/error.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
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

Keep this enum flat. Do not include raw provider responses, request bodies, headers, bearer tokens, API keys, SDK debug output, or any credential-bearing value in error payloads.

- [ ] **Step 3: Run compiler to verify expected missing modules**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: FAIL with unresolved module files for `provider`, `service`, or `store`. This confirms the new module is wired.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/provider_api_key/mod.rs src-tauri/src/provider_api_key/error.rs
git commit -m "feat(provider-api-key): add service error boundary"
```

## Task 2: Secret Wrapper And Collaborator Traits

**Files:**
- Create: `src-tauri/src/provider_api_key/store.rs`
- Create: `src-tauri/src/provider_api_key/provider.rs`

- [ ] **Step 1: Add `ProviderApiKey`**

Create `src-tauri/src/provider_api_key/store.rs` with a backend-only secret wrapper:

```rust
use std::fmt;

use crate::{domain::provider::GpuCloudProviderId, shared::AppFuture};

use super::error::ProviderApiKeyError;

#[derive(Clone, PartialEq, Eq)]
pub struct ProviderApiKey {
    raw: String,
}

impl ProviderApiKey {
    pub fn new(raw: impl Into<String>) -> Result<Self, ProviderApiKeyError> {
        let raw = raw.into();

        if raw.trim().is_empty() {
            return Err(ProviderApiKeyError::StoredProviderApiKeyInvalid);
        }

        Ok(Self { raw })
    }

    pub fn expose_secret(&self) -> &str {
        &self.raw
    }
}

impl fmt::Debug for ProviderApiKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderApiKey")
            .field("raw", &"<redacted>")
            .finish()
    }
}
```

Do not derive or implement `Serialize`, `Deserialize`, or Specta `Type` for `ProviderApiKey`.

- [ ] **Step 2: Add the storage trait**

In `src-tauri/src/provider_api_key/store.rs`, add:

```rust
pub trait ProviderApiKeyStore: Send + Sync {
    fn has_key<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
    ) -> AppFuture<'a, Result<bool, ProviderApiKeyError>>;

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

Store implementations must map read/write/remove/check failures to `ProviderApiKeyError::SecureKeyringUnavailable`. Invalid stored key material must be returned as `ProviderApiKeyError::StoredProviderApiKeyInvalid`.

- [ ] **Step 3: Add the provider identity trait**

Create `src-tauri/src/provider_api_key/provider.rs`:

```rust
use crate::{
    domain::{provider::GpuCloudProviderId, shared::ApiKeySetup},
    shared::AppFuture,
};

use super::{error::ProviderApiKeyError, store::ProviderApiKey};

pub trait ProviderIdentityValidator: Send + Sync {
    fn validate_identity<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        api_key: &'a ProviderApiKey,
    ) -> AppFuture<'a, Result<ApiKeySetup, ProviderApiKeyError>>;
}
```

Provider implementations must normalize provider failures into `ProviderUnauthorized`, `ProviderRateLimited`, `ProviderTimeout`, or `ProviderRequestFailed { message }` with UI-safe messages only.

- [ ] **Step 4: Add focused wrapper and trait tests**

Add tests in `store.rs` for:

- `ProviderApiKey` accepts non-blank values.
- `ProviderApiKey` rejects blank values.
- `ProviderApiKey` debug output is redacted and does not contain the raw secret.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/provider_api_key/store.rs src-tauri/src/provider_api_key/provider.rs
git commit -m "feat(provider-api-key): add key collaborators"
```

## Task 3: Service Skeleton And Setup Validation

**Files:**
- Create: `src-tauri/src/provider_api_key/service.rs`

- [ ] **Step 1: Add service structure**

Create `src-tauri/src/provider_api_key/service.rs`:

```rust
use crate::{
    domain::{provider::GpuCloudProviderId, shared::ApiKeySetup},
};

use super::{
    error::ProviderApiKeyError,
    provider::ProviderIdentityValidator,
    store::{ProviderApiKey, ProviderApiKeyStore},
};

pub struct ProviderApiKeyService<S, V> {
    store: S,
    validator: V,
}

impl<S, V> ProviderApiKeyService<S, V>
where
    S: ProviderApiKeyStore,
    V: ProviderIdentityValidator,
{
    pub fn new(store: S, validator: V) -> Self {
        Self { store, validator }
    }
}
```

- [ ] **Step 2: Add `ApiKeySetup` validation**

In `service.rs`, add a private validation helper:

```rust
fn validate_api_key_setup(setup: ApiKeySetup) -> Result<ApiKeySetup, ProviderApiKeyError> {
    if setup.email.trim().is_empty()
        || setup.username.trim().is_empty()
        || setup.key_display_name.trim().is_empty()
        || setup.email.chars().any(char::is_control)
        || setup.username.chars().any(char::is_control)
        || setup.key_display_name.chars().any(char::is_control)
    {
        return Err(ProviderApiKeyError::ProviderIdentityResponseInvalid);
    }

    Ok(setup)
}
```

- [ ] **Step 3: Add validation tests**

In `service.rs`, add focused tests for:

- accepting non-blank `ApiKeySetup` fields;
- rejecting blank `email`, `username`, and `key_display_name`;
- rejecting control characters in each `ApiKeySetup` field.

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml provider_api_key
```

Expected: PASS for the wrapper tests and setup validation tests now that all module files exist.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/provider_api_key/service.rs
git commit -m "feat(provider-api-key): validate setup identity"
```

## Task 4: Service Methods And Fakes

**Files:**
- Modify: `src-tauri/src/provider_api_key/service.rs`

- [ ] **Step 1: Add `has_key` and `read_key`**

Implement:

```rust
pub async fn has_key(
    &self,
    provider_id: GpuCloudProviderId,
) -> Result<bool, ProviderApiKeyError> {
    self.store.has_key(provider_id).await
}

pub async fn read_key(
    &self,
    provider_id: GpuCloudProviderId,
) -> Result<ProviderApiKey, ProviderApiKeyError> {
    self.store
        .read_key(provider_id)
        .await?
        .ok_or(ProviderApiKeyError::ProviderSetupIncomplete)
}
```

These methods must not call provider identity validation.

- [ ] **Step 2: Add `write_key`**

Implement:

```rust
pub async fn write_key(
    &self,
    provider_id: GpuCloudProviderId,
    api_key: ProviderApiKey,
) -> Result<ApiKeySetup, ProviderApiKeyError> {
    if self.store.has_key(provider_id).await? {
        return Err(ProviderApiKeyError::ProviderSetupAlreadyExists);
    }

    let setup = self.validator.validate_identity(provider_id, &api_key).await?;
    let setup = validate_api_key_setup(setup)?;

    self.store.write_key(provider_id, &api_key).await?;

    Ok(setup)
}
```

This order is required: check whether setup already exists, reject existing setup before provider validation, validate provider identity, validate returned setup fields, then write storage. Do not mutate storage when setup already exists, provider validation fails, or setup validation fails.

- [ ] **Step 3: Add `remove_key`**

Implement:

```rust
pub async fn remove_key(
    &self,
    provider_id: GpuCloudProviderId,
) -> Result<(), ProviderApiKeyError> {
    if !self.store.has_key(provider_id).await? {
        return Err(ProviderApiKeyError::ProviderSetupIncomplete);
    }

    self.store.remove_key(provider_id).await
}
```

This method must not call provider identity validation.

- [ ] **Step 4: Add `validate_identity`**

Implement:

```rust
pub async fn validate_identity(
    &self,
    provider_id: GpuCloudProviderId,
) -> Result<ApiKeySetup, ProviderApiKeyError> {
    let api_key = self.read_key(provider_id).await?;
    let setup = self.validator.validate_identity(provider_id, &api_key).await?;

    validate_api_key_setup(setup)
}
```

- [ ] **Step 5: Add test fakes**

In the `service.rs` test module, add fake store and fake validator implementations using simple call counters and configured results. The fakes should let tests assert:

- whether storage was touched;
- whether validation was called;
- operation order for `write_key`;
- existing setup rejection before provider validation;
- stored key presence by `GpuCloudProviderId`.

Keep fakes local to tests. Do not add production in-memory storage.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/provider_api_key/service.rs
git commit -m "feat(provider-api-key): add key lifecycle service"
```

## Task 5: Required Service Test Matrix

**Files:**
- Modify: `src-tauri/src/provider_api_key/service.rs`
- Modify if needed: `src-tauri/src/provider_api_key/store.rs`

- [ ] **Step 1: Cover `has_key` behavior**

Add tests for:

- `has_key` returns `true` from storage.
- `has_key` returns `false` from storage.
- `has_key` maps storage failure to `SecureKeyringUnavailable`.
- `has_key` does not call provider identity validation.

- [ ] **Step 2: Cover `read_key` behavior**

Add tests for:

- `read_key` returns a stored key.
- `read_key` returns `ProviderSetupIncomplete` when no key exists.
- `read_key` maps invalid stored key to `StoredProviderApiKeyInvalid`.
- `read_key` does not call provider identity validation.

- [ ] **Step 3: Cover `write_key` success behavior**

Add tests for:

- `write_key` validates provider identity before writing storage.
- `write_key` stores a non-blank `ProviderApiKey` only after provider identity validation succeeds.
- `write_key` returns the validated `ApiKeySetup` after storing.
- blank raw key input cannot be written because `ProviderApiKey` construction rejects it before `write_key`.

- [ ] **Step 4: Cover `write_key` failure behavior**

Add tests for:

- `write_key` returns `ProviderSetupAlreadyExists` without provider validation when a key already exists.
- `write_key` maps storage check failure to `SecureKeyringUnavailable` without provider validation.
- `write_key` maps storage write failure to `SecureKeyringUnavailable`.
- `write_key` maps unauthorized provider validation to `ProviderUnauthorized` without mutating storage.
- `write_key` maps rate-limited provider validation to `ProviderRateLimited` without mutating storage.
- `write_key` maps provider timeout to `ProviderTimeout` without mutating storage.
- `write_key` maps other UI-safe provider failures to `ProviderRequestFailed { message }` without mutating storage.
- `write_key` rejects blank or control-character `ApiKeySetup` fields as `ProviderIdentityResponseInvalid` without mutating storage.

- [ ] **Step 5: Cover `remove_key` behavior**

Add tests for:

- `remove_key` removes an existing provider API key.
- `remove_key` returns `ProviderSetupIncomplete` when no key exists.
- `remove_key` maps storage remove/check failure to `SecureKeyringUnavailable`.
- `remove_key` does not call provider identity validation.

- [ ] **Step 6: Cover `validate_identity` behavior**

Add tests for:

- `validate_identity` validates identity with the stored key.
- `validate_identity` returns `ApiKeySetup` with email, username, and key display name.
- `validate_identity` returns `ProviderSetupIncomplete` when the key is missing.
- `validate_identity` maps unauthorized provider validation to `ProviderUnauthorized`.
- `validate_identity` maps rate-limited provider validation to `ProviderRateLimited`.
- `validate_identity` maps provider timeout to `ProviderTimeout`.
- `validate_identity` maps other UI-safe provider failures to `ProviderRequestFailed { message }`.
- `validate_identity` rejects blank or control-character `ApiKeySetup` fields as `ProviderIdentityResponseInvalid`.

- [ ] **Step 7: Run focused tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml provider_api_key
```

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/provider_api_key/service.rs src-tauri/src/provider_api_key/store.rs
git commit -m "test(provider-api-key): cover key lifecycle behavior"
```

## Task 6: Full Verification

**Files:**
- No code changes expected.

- [ ] **Step 1: Run native backend tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

- [ ] **Step 2: Run Rust formatting check**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

- [ ] **Step 3: Run Clippy**

Run:

```bash
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

- [ ] **Step 4: Confirm no command codegen is needed**

No `bun run codegen:commands` is required because this plan does not add Tauri commands, change command signatures, export new Specta types, or edit `src/generated/commands.ts`.

- [ ] **Step 5: Commit any verification-only fixes**

If verification exposes compile, formatting, or lint issues, fix only the provider API key module and commit:

```bash
git add src-tauri/src/provider_api_key src-tauri/src/lib.rs
git commit -m "fix(provider-api-key): satisfy native verification"
```
