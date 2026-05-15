## 1. Provider Error Mapping

- [x] 1.1 Add stable `ProviderClientError` variants for provider rate limiting and provider request rejection.
- [x] 1.2 Map RunPod REST provisioning statuses into provider-local errors without exposing RunPod-specific codes downstream.
- [x] 1.3 Map RunPod inventory HTTP statuses, including non-authentication `4xx`, into provider-local errors.
- [x] 1.4 Map RunPod GraphQL identity and inventory errors into provider-local errors.
- [x] 1.5 Add provider adapter tests for unauthorized, rate limited, request rejected, resource missing, conflict, response invalid, and indeterminate outcomes.

## 2. Registry and Use-Case Mapping

- [x] 2.1 Update provider registry mappings from provider-local failures into Provider Setup errors.
- [x] 2.2 Update provider registry mappings from provider-local failures into Workspace Setup errors.
- [x] 2.3 Update provider registry mappings from provider-local failures into Workspace Provisioning errors.
- [x] 2.4 Add mapping tests that assert rate limiting and request rejection are not collapsed into generic provider unavailability.

## 3. Command Contract and Frontend Presentation

- [x] 3.1 Add stable `NativeCommandError` codes, reasons, retryability, and recovery actions for provider request rejection and rate limiting.
- [x] 3.2 Update command error mapping tests for provider request rejection and rate limiting.
- [x] 3.3 Regenerate generated TypeScript command bindings.
- [x] 3.4 Update frontend error presentation copy for the new provider error codes.
- [x] 3.5 Ensure command responses and generated bindings do not expose provider transport details, raw provider payloads, Provider API Keys, or worker bearer tokens.

## 4. Native Logging

- [x] 4.1 Keep command failure logging limited to stable UI-safe command metadata.
- [x] 4.2 Add logging coverage that provider request rejection logs do not include secrets, raw provider payloads, or provider-specific error details.

## 5. Documentation and Verification

- [x] 5.1 Update OpenSpec proposal, design, specs, and tasks to match the narrowed implementation.
- [x] 5.2 Run `cargo test`.
- [x] 5.3 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 5.4 Run `cargo fmt`.
- [x] 5.5 Run `bun run build`.
- [x] 5.6 Run `bun run lint --fix`.
