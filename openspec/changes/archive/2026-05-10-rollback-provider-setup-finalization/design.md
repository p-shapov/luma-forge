## Context

Provider setup currently validates the submitted key, writes it to the secure keyring, then re-reads and validates the stored key before returning success. This protects the invariant that successful setup is derived from durable keyring state, but it also creates a post-write failure window: re-read or second provider validation can fail after the key has already been stored.

That failure window currently returns an error while leaving a keyring entry behind. A retry can then observe the stored key and reject with `provider_setup_already_exists`, even though the original setup command failed.

## Goals / Non-Goals

**Goals:**

- Keep setup success derived from the re-read stored key.
- Make first-time setup effectively transactional for normal post-write finalization failures.
- Preserve retry behavior when rollback succeeds.
- Expose a distinct recovery-required error when rollback fails and partial local setup may remain.
- Keep provider secrets out of command responses, logs, diagnostics, and generated frontend types.

**Non-Goals:**

- Do not add key rotation or replacement setup.
- Do not revoke provider-side RunPod API keys.
- Do not introduce a persisted provider setup status table.
- Do not create a full Factory Reset flow.
- Do not add setup-specific frontend recovery presentation until a Provider Setup UI error path exists.

## Decisions

### Roll back the newly written key on finalization failure

After `replace_api_key()` succeeds, setup finalization still requires `read_api_key()` and provider identity validation using the stored key. If either finalization step fails, Provider Setup should call `delete_api_key()` for the same provider before returning.

```
submitted key validates
        │
        ▼
replace_api_key succeeds
        │
        ▼
re-read + validate stored key
        │
        ├─ success ───────────────▶ return setup
        │
        └─ failure
             │
             ├─ delete succeeds ─▶ return original finalization error
             └─ delete fails ────▶ return recovery-required error
```

This keeps the existing two-validation contract while making the common transient failure path retry-safe.

Alternative considered: remove the final re-read and second validation. That would avoid the post-write provider failure window, but it weakens the durable-state invariant and conflicts with the existing setup lifecycle contract.

### Return the original error when rollback succeeds

If finalization fails and rollback succeeds, the command should return the original finalization error. For example, a provider API timeout during stored-key validation should still surface as `provider_api_unavailable`, and a secure keyring re-read failure should still surface as `secure_keyring_unavailable`.

This keeps normal retry semantics unchanged: the user sees the actual reason setup did not complete, and because rollback succeeded, retry is not blocked by the failed attempt.

Alternative considered: always return a generic rollback-related error after any post-write failure. That hides useful failure information and would make ordinary transient provider failures look like local recovery problems.

### Add a distinct recovery-required error when rollback fails

If finalization fails and rollback deletion also fails, the Native Layer cannot honestly claim setup is complete or that the failed attempt was cleaned up. Provider Setup should return a dedicated use-case error that maps to a generated `provider_setup_recovery_required` command error code.

The error should be UI-safe and must not include the submitted key, stored key, provider transport details, or keyring diagnostics. It should indicate that local provider setup may require explicit recovery before setup can be retried.

The generated command error should be non-retryable for the same setup command. The recovery path is a separate action, such as deleting local provider setup once keyring access is available or using a future factory reset flow.

Alternative considered: return `secure_keyring_unavailable` when rollback fails. That preserves the public error enum, but it loses the critical distinction between "nothing was stored" and "partial local setup may remain."

### Keep rollback scoped to first-time setup writes

Rollback should only remove the key that setup just wrote after observing no existing setup at the start of the serialized operation. Existing setup still rejects before provider validation and must not be removed by a failed repeated setup attempt.

The existing provider setup coordinator remains the serialization boundary for setup/delete operations. This change does not need a new lock or storage transaction primitive.

Alternative considered: compare the stored key with the submitted key before rollback. That adds secret comparison complexity and is unnecessary inside the existing serialized first-time setup path.

## Risks / Trade-offs

- New public error code affects exhaustive frontend matches -> update generated bindings and command error handling tests in the same implementation change.
- No current Provider Setup UI error renderer exists -> do not add dead frontend presentation helpers; defer setup-specific recovery copy until the UI path exists.
- Rollback delete can fail because the keyring is unavailable -> return `provider_setup_recovery_required` and leave recovery to explicit delete/factory-reset behavior.
- A provider outage during the second validation still causes setup failure -> this is intentional because success must be derived from stored state.
- Delete-on-finalization-failure may remove a key that was successfully written but temporarily unverifiable -> acceptable for first-time setup because the user received failure and expects retry, not hidden completion.

## Migration Plan

No durable data migration is required. The change affects runtime behavior for future failed setup attempts and expands the generated command error code union.

Existing hidden partial states are not automatically migrated. Users already in that state can use the existing delete setup path when the keyring entry is accessible.

## Open Questions

None.
