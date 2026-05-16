## Context

LumaForge provisions provider-owned RunPod resources through native Rust services. The RunPod adapter reduces HTTP, transport, parsing, and provider response failures into the small `ProviderClientError` enum before the provider registry maps those failures into setup, workspace setup, or workspace provisioning use-case errors.

That boundary is useful because React and domain services should not depend on RunPod status codes, response envelopes, or message strings. The missing piece is that provider rate limiting and non-authentication request rejection need stable LumaForge-owned variants instead of being collapsed into broad provider unavailability or invalid-response errors.

The change must preserve existing security boundaries: Provider API Keys, worker bearer tokens, raw request bodies, raw response bodies, and provider transport details must not appear in command responses, Workspace metadata, or native command logs.

## Goals / Non-Goals

**Goals:**

- Keep provider-specific response interpretation inside the provider adapter.
- Add stable provider error variants for rate limiting and provider request rejection.
- Map RunPod REST and GraphQL outcomes into provider-local errors without exposing RunPod-specific codes downstream.
- Make command retryability and recovery actions distinguish provider unavailability, rate limiting, and request rejection.
- Keep command-facing errors UI-safe and stable for React.

**Non-Goals:**

- Do not make RunPod error codes, message strings, or response envelopes part of LumaForge domain contracts.
- Do not expose raw provider payloads, bearer headers, Provider API Keys, worker tokens, stack traces, or keyring details.
- Do not add a provider diagnostic payload, operation-context wrapper, or separate classification abstraction.
- Do not change Workspace Provisioning lifecycle policy or mark additional Workspaces failed as part of this change.
- Do not implement provider resource discovery or reconciliation.
- Do not add a hosted backend service.
- Do not add local ML inference.

## Decisions

### Extend the Existing Provider Error Enum

Provider adapters will continue returning `ProviderClientError`. The enum gains explicit variants for provider rate limiting and provider request rejection:

- `RateLimited`
- `RequestRejected`

Existing variants continue to cover authorization failure, provider unavailability, invalid provider responses, missing resources, conflicts, and indeterminate provider mutation results.

Alternative considered: add a separate classification type with operation metadata and diagnostic payloads. That added indirection without being needed for the current recovery behavior, so the implementation keeps the single provider error enum.

### Keep RunPod Mapping Inside the RunPod Adapter

The RunPod adapter may inspect HTTP status classes and GraphQL error messages as provider-local hints. It must output `ProviderClientError` variants and must not require downstream modules to understand RunPod response codes or message strings.

REST provisioning resource responses map by status:

- `401` and `403` -> `Unauthorized`
- `404` -> `NotFound`
- `429` -> `RateLimited`
- `409` -> `Conflict`
- `408` and `504` -> `Indeterminate`
- other `4xx` -> `RequestRejected`
- other non-success statuses -> `ApiUnavailable`

RunPod inventory HTTP responses use the same intent at the inventory boundary:

- `401` and `403` -> `Unauthorized`
- `429` -> `RateLimited`
- other `4xx` -> `RequestRejected`
- other non-success statuses -> `ApiUnavailable`

GraphQL errors that look authentication-related map to `Unauthorized`; other GraphQL errors map to `RequestRejected`.

Alternative considered: hardcode RunPod datacenter deny lists in Workspace Setup. That may still be useful later if RunPod exposes reliable capability data, but it is separate from provider failure classification.

### Keep Command DTO Shape Stable

The existing `NativeCommandError` shape remains stable: `code`, `message`, `retryable`, `field`, `reason`, and `recovery_action`.

The change adds stable codes and reasons for:

- `provider_rate_limited`
- `provider_request_rejected`

Rate limiting is retryable. Request rejection is not retryable without changing the request or placement selection.

Alternative considered: add a provider details object to command errors. That would increase UI and logging surface area and is not necessary for the current behavior.

### Do Not Change Provisioning Lifecycle Policy

Workspace Provisioning continues to surface provider failures through existing command error flow and retained metadata behavior. This change does not introduce new durable failure transitions for request rejection, missing resources, or indeterminate mutations.

Those lifecycle policies can be designed separately if the app needs stronger reconciliation or cleanup semantics later.

## Risks / Trade-offs

- RunPod GraphQL error messages are unstable -> use them only to distinguish obvious authentication failures; all other GraphQL errors collapse to `RequestRejected`.
- More provider error variants increase mapping surface -> keep mapping centralized in the provider registry and cover it with tests.
- Inventory `4xx` responses are now request rejections -> this gives better recovery guidance than provider unavailability, but still intentionally avoids exposing RunPod-specific details.

## Migration Plan

1. Add `RateLimited` and `RequestRejected` to `ProviderClientError`.
2. Map RunPod REST and GraphQL outcomes into the provider error enum.
3. Update provider registry mappings for provider setup, workspace setup, and workspace provisioning.
4. Add stable native command codes, reasons, retryability, and recovery actions for rate limiting and request rejection.
5. Regenerate generated command bindings and update frontend error presentation copy.
6. Verify with `cargo test`, `cargo clippy --fix --allow-dirty --allow-staged`, `cargo fmt`, `bun run build`, and `bun run lint --fix`.

Rollback is straightforward before release: revert the provider error enum additions, mapping changes, generated bindings, and frontend copy. Persisted Workspace metadata does not require migration.

## Open Questions

- Should RunPod network-volume capability filtering be added later if provider inventory exposes reliable placement capability data?
- Should Workspace Provisioning get a separate reconciliation policy for indeterminate mutations in a future change?
