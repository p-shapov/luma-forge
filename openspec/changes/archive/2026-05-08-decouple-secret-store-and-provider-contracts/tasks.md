## 1. Secret Store Error Boundary

- [x] 1.1 Add `SecretStoreError` to `src-tauri/src/secrets.rs` with variants for unavailable secure keyring access and invalid stored provider API key data.
- [x] 1.2 Update the `SecretStore` trait and `KeyringSecretStore` implementation to return `SecretStoreError` instead of `ProviderSetupError`.
- [x] 1.3 Add independent mappings from `SecretStoreError` into `ProviderSetupError` and `WorkspaceSetupError`, preserving current command error behavior.
- [x] 1.4 Remove Workspace Setup's conversion dependency on `ProviderSetupError` for secret store failures.
- [x] 1.5 Update provider setup, workspace setup, and provider client test doubles to use `SecretStoreError`.

## 2. Shared Provider Command DTO Boundary

- [x] 2.1 Add a neutral shared native contract module for provider command DTOs.
- [x] 2.2 Move command-facing `GpuCloudProviderId` and its domain conversion implementations from Provider Setup contracts into the shared provider contract module.
- [x] 2.3 Update Provider Setup contracts and services to import the shared `GpuCloudProviderId`.
- [x] 2.4 Update Workspace Setup contracts, workspace contracts, workspace catalog persistence, and tests to import the shared `GpuCloudProviderId` directly.
- [x] 2.5 Verify no non-Provider Setup module imports shared provider command DTOs from `provider_setup`.

## 3. Verification

- [x] 3.1 Run `cargo test`.
- [x] 3.2 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 3.3 Run `cargo fmt`.
- [x] 3.4 Verify generated TypeScript still exports `GpuCloudProviderId` with the same `runpod` value and no Provider API Key exposure.
