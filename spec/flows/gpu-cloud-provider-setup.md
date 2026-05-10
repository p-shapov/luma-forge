# GPU Cloud Provider Setup

## Goal

Configure one supported GPU Cloud Provider and store one validated Provider API Key for that provider.

Setup is successful only after the key is validated, stored in the local secure keyring, re-read from the keyring, and used to derive a redacted provider setup status.

## Scope

- This flow establishes local provider setup state from a provider-scoped secure keyring entry.
- It stores the submitted Provider API Key only in the secure keyring.
- It does not persist a separate selected/default provider configuration.

## Non-goals

- It does not create Workspace metadata or Provider Resources.

## Invariants

- The Client (React) never persists, logs, or receives the Provider API Key after submission.
- The Provider API Key is stored only in the secure keyring and is never written to local metadata or logs.

## Actors

- User
- Client (React)
- Native Layer (Rust / Tauri)
- Provider

## Preconditions

- The application is running locally and knows the supported GPU Cloud Providers.
- Native Layer (Rust / Tauri) can access the local secure keyring.

## Main Flow

1. User sees GPU Cloud Provider setup.
2. Client (React) requests setup status.
3. Native Layer (Rust / Tauri):
   - checks secure keyring
   - if a key exists, validates it with Provider identity
   - returns live redacted provider setup status
4. Client (React):
   - renders current setup state
   - allows setup only if no complete setup exists

---

5. User submits setup:
   - selected provider
   - Provider API Key (ephemeral, not persisted in Client)
6. Client (React) sends a single setup request to Native Layer.

---

7. Native Layer (Rust / Tauri) validates request:
   - provider is supported
   - no complete setup already exists

   If validation fails -> reject (no mutation)

---

8. Native Layer validates API key with Provider.
   If validation fails -> reject (no mutation)

---

9. Native Layer performs mutations in strict order:
   - 9.1 Store API key in secure keyring

   If step 9.1 fails -> reject (no mutation)

---

10. Native Layer re-reads setup state:

   - key presence in keyring
   - provider identity for the stored key

   Ensures setup success is derived from durable keyring state.

---

11. Native Layer returns redacted provider status:

   - provider identity
   - key fingerprint
   - validation status

---

12. Client (React) updates UI based on returned status.

## Success Result

- Exactly one Provider API Key is stored in the local secure keyring for the requested GPU Cloud Provider.
- The Native Layer derives completed setup from the stored key and Provider identity.
- Native Layer (Rust / Tauri) returns only redacted provider status to Client (React).

## Failure Handling

The Native Layer must fail closed: no partial setup may be reported as successful.

- Unsupported provider:
  - Native behavior: Rejects before provider validation.
  - Mutation guarantee: No keyring mutation.
  - Client behavior: Shows validation error.
- Invalid or empty API key:
  - Native behavior: Rejects before provider validation.
  - Mutation guarantee: No keyring mutation.
  - Client behavior: Shows validation error.
- Existing setup:
  - Native behavior: Rejects or returns existing redacted status.
  - Mutation guarantee: No keyring mutation.
  - Client behavior: Blocks setup UI and shows redacted status.
- Provider validation failure:
  - Native behavior: Rejects after validation attempt.
  - Mutation guarantee: No keyring mutation.
  - Client behavior: Shows invalid key error.
- Provider API timeout/network error:
  - Native behavior: Rejects as transient failure.
  - Mutation guarantee: No keyring mutation.
  - Client behavior: Shows network error, allows retry.
- Secure keyring read failure:
  - Native behavior: Rejects before setup can start.
  - Mutation guarantee: No keyring mutation.
  - Client behavior: Blocks setup UI.
- Secure keyring write failure:
  - Native behavior: Rejects setup.
  - Mutation guarantee: Completed setup is not reported.
  - Client behavior: Shows local storage error.
- Redacted status read failure:
  - Native behavior: Rejects status request.
  - Mutation guarantee: No mutation.
  - Client behavior: Shows setup status unavailable.

## Idempotency

The setup resource is uniquely identified by the requested provider.

After a successful setup, the Native Layer (Tauri / Rust) rejects any subsequent setup request, regardless of whether the submitted key matches the existing key.

The key fingerprint is used only for redacted status and diagnostics, not for accepting repeated setup requests.

## Cleanup / Rollback

Client clears temporary Provider API Key input after success, failure, timeout, or cancellation.

Recovery from invalid local provider/key state is handled by Factory Reset.

## See Also

[GPU Cloud Provider](../ubiquitous-language/gpu-cloud-provider.md)
[Provider API Key](../ubiquitous-language/provider-api-key.md)
[Factory Reset](../ubiquitous-language/factory-reset.md)
