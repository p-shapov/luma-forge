## 1. Domain Contract

- [x] 1.1 Add structured provisioning failure domain types with stable code, phase, source, retryability, recovery action, and optional sanitized diagnostic fields.
- [x] 1.2 Add optional persisted failure detail to Workspace metadata with serde defaults for existing workspace rows.
- [x] 1.3 Update Workspace domain tests for default draft workspaces, failed workspace serialization, and legacy metadata without failure detail.

## 2. Progress and Command Bindings

- [x] 2.1 Update Workspace Provisioning Progress derivation so failed workspaces expose structured failure detail or a generic legacy fallback.
- [x] 2.2 Remove the existing progress `message` field and update generated Specta remote types accordingly.
- [x] 2.3 Regenerate generated TypeScript command bindings and update frontend contract consumers.

## 3. Provisioning Failure Mapping

- [x] 3.1 Add centralized helpers that convert terminal provider resource observations into structured failure detail.
- [x] 3.2 Add centralized helpers that convert terminal worker failures and unrecoverable worker API errors into structured failure detail without leaking raw diagnostics.
- [x] 3.3 Add centralized helpers for native unsafe-continuation failures such as missing worker token, readiness validation failure, cancellation cleanup failure, and unrecoverable indeterminate mutation state.
- [x] 3.4 Update every provisioning service path that sets lifecycle state to `failed` to persist failure detail in the same Workspace update.

## 4. Provider Error Lifecycle Semantics

- [x] 4.1 Preserve provider rate limiting, provider API unavailability, operation conflict, and safe request rejection as UI-safe `NativeCommandError` responses without changing Workspace lifecycle state.
- [x] 4.2 Persist `failed` lifecycle plus structured failure detail when provider failure or observation leaves Native unable to identify one safe continuation path.
- [x] 4.3 Add tests that prove provider command failures preserve existing Workspace metadata and snapshots when no new authoritative observation exists.
- [x] 4.4 Add tests that prove unsafe provider continuation persists failed lifecycle, structured failure detail, and cleanup metadata.

## 5. Frontend Integration

- [x] 5.1 Update provisioning UI state handling to classify failed provisioning from structured Native data instead of parsing free-form message text.
- [x] 5.2 Render recovery affordances from failure recovery action while keeping provider setup recovery, placement reselection, retry, and cleanup paths distinct.
- [x] 5.3 Ensure frontend code never displays secret-bearing or raw provider/worker diagnostic fields.

## 6. Verification

- [x] 6.1 Run `cargo test`.
- [x] 6.2 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 6.3 Run `cargo fmt`.
- [x] 6.4 Run `bun run build`.
- [x] 6.5 Run `bun run lint --fix`.
- [x] 6.6 Run OpenSpec validation for `add-provisioning-failure-contract`.
