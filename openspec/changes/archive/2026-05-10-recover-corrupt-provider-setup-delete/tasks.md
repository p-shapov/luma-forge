## 1. Secret Store Boundary

- [x] 1.1 Add a raw provider keyring-entry presence method to `SecretStore` that checks whether an entry exists without parsing the stored value as `ProviderApiKey`.
- [x] 1.2 Implement the raw presence method for `KeyringSecretStore`, mapping missing entries to `Ok(false)` and keyring backend failures to `SecureKeyringUnavailable`.
- [x] 1.3 Update in-memory test secret stores to model valid entries, corrupt entries, missing entries, and keyring access failures.

## 2. Provider Setup Delete Behavior

- [x] 2.1 Update `ProviderSetupService::delete_setup` to use raw keyring-entry presence before deleting.
- [x] 2.2 Preserve `provider_setup_incomplete` when no provider keyring entry exists.
- [x] 2.3 Preserve `secure_keyring_unavailable` when entry lookup or deletion fails.
- [x] 2.4 Keep setup status, setup creation, workspace setup, and provider clients on strict parsed reads.

## 3. Verification

- [x] 3.1 Add a provider setup service test proving delete succeeds and clears the entry when stored key material is corrupt.
- [x] 3.2 Keep or update existing tests for successful delete, missing setup delete, and keyring delete failure.
- [x] 3.3 Run `cargo test`.
- [x] 3.4 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 3.5 Run `cargo fmt`.
