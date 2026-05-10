## 1. Provider Setup Error Contract

- [x] 1.1 Add a provider setup use-case error for recovery-required partial setup state after rollback failure.
- [x] 1.2 Map the new use-case error to a generated `provider_setup_recovery_required` native command error code.
- [x] 1.3 Mark the new command error as not retryable for the same setup command and keep its message UI-safe.
- [x] 1.4 Regenerate or update generated TypeScript command bindings so `NativeCommandErrorCode` includes `provider_setup_recovery_required`.

## 2. Setup Finalization Rollback

- [x] 2.1 Refactor setup creation so post-write re-read and stored-key validation failures flow through a rollback helper.
- [x] 2.2 Delete the newly written provider keyring entry when stored-key re-read fails after a first-time setup write.
- [x] 2.3 Delete the newly written provider keyring entry when stored-key provider identity validation fails after a first-time setup write.
- [x] 2.4 Return the original finalization error when rollback deletion succeeds.
- [x] 2.5 Return the recovery-required provider setup error when rollback deletion fails.

## 3. Frontend Scope

- [x] 3.1 Regenerate the frontend command contract so `provider_setup_recovery_required` is available to React.
- [x] 3.2 Confirm no Provider Setup UI error presentation path exists yet and avoid adding unused frontend presentation helpers.

## 4. Tests

- [x] 4.1 Update the existing stored-key re-read failure test to expect rollback and no stored key after the failed setup.
- [x] 4.2 Add a setup test proving stored-key provider validation failure rolls back the newly written key.
- [x] 4.3 Add a setup test proving rollback deletion failure returns `provider_setup_recovery_required` and does not report setup success.
- [x] 4.4 Add command error mapping tests for `provider_setup_recovery_required`, including retryability and UI-safe message behavior.
- [x] 4.5 Add or update frontend tests for the new command error branch if the setup UI has error-state coverage.

## 5. Verification

- [x] 5.1 Run `cargo test`.
- [x] 5.2 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 5.3 Run `cargo fmt`.
- [x] 5.4 Run `bun run build`.
- [x] 5.5 Run `bun run lint --fix`.
