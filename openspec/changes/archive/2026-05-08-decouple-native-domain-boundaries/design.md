## Context

The native layer already has separate modules for domain models, bundled catalogs, provider clients, application services, persistence, and Tauri commands. Some types still cross those boundaries in the wrong direction:

- domain models derive `Serialize`, `Deserialize`, and `specta::Type`, tying core domain data to command and frontend binding concerns
- workspace command contracts import bundled catalog DTOs, so workspace setup depends on catalog infrastructure ownership
- bundled profile DTOs own RunPod-specific config shapes, even though RunPod-specific configuration belongs to the provider boundary
- provider inventory lookup maps authorization failures as transient provider API failures

The next major native flow, Workspace Provisioning, will reuse workflows, placement plans, provider profiles, provider clients, command responses, and persistence snapshots. Tightening these boundaries before provisioning avoids copying the same leaks into a larger workflow.

## Goals / Non-Goals

**Goals:**

- Keep `src-tauri/src/domain` provider-agnostic and independent from command binding, serialization, persistence, Tauri runtime, and provider transport concerns.
- Move RunPod-specific profile/config contract types under `src-tauri/src/provider/runpod`.
- Keep bundled catalog reading as infrastructure that parses static bundled data and maps it into application/provider/domain-facing types.
- Keep command request/response DTOs under command or use-case contract modules and map them explicitly to application/domain types.
- Correct provider inventory authorization failures so invalid or revoked keys surface as provider setup recovery errors.
- Preserve current Workspace Setup creation semantics, including duplicate Workspace UUID rejection and deferred live placement availability validation.

**Non-Goals:**

- Do not move RunPod-specific config types into domain.
- Do not introduce a new provider abstraction framework beyond the existing registry/gateway pattern.
- Do not change Workspace Setup to perform live provider inventory validation during workspace creation.
- Do not change frontend UX beyond generated type/import updates needed by renamed or relocated DTOs.
- Do not add new runtime dependencies.

## Decisions

### Keep Domain Pure And Provider-Agnostic

Domain structs and enums will stop deriving `serde` and `specta::Type`. Domain types remain plain Rust models for native business concepts and invariants. Any type that must cross the Tauri command boundary will be represented by a DTO with explicit conversion.

Alternative considered: leave derives on domain models and treat them as harmless metadata. Rejected because generated binding requirements and serialization details would continue to constrain domain changes and encourage using domain models as command contracts.

### Put RunPod Profile Config Under Provider Boundary

RunPod-specific profile config types such as provisioning profile config, endpoint profile config, and serverless scaling config will live under `provider::runpod`. Generic profile shells may remain domain/application-owned, but provider-specific config payloads are owned by the provider module.

Alternative considered: move RunPod config into `domain::profiles`. Rejected because the domain must not know RunPod template identifiers, RunPod scaling fields, or RunPod-specific resource configuration.

### Make Bundled Catalog An Infrastructure Adapter

The bundled catalog module will parse bundled JSON/YAML into catalog records and map provider-specific profile payloads through provider-owned contract types. It may return command-safe/application-safe catalog snapshots, but it must not be the owner of types used by workspace contracts.

Alternative considered: keep bundled profile DTOs as the shared profile representation. Rejected because that makes one infrastructure source define the workspace API and persistence shape.

### Use Explicit DTO Mapping At Command Boundaries

Command request/response types will derive `Serialize`, `Deserialize`, and `specta::Type`. Application services can return application snapshots or domain types, and command handlers or contract mappers convert those into UI-safe DTOs.

This keeps generated frontend bindings stable as an explicit boundary, while allowing domain and provider internals to evolve independently.

### Classify Inventory Authorization Failures Separately

RunPod inventory lookup will classify `401` and `403` as provider authorization failures, matching identity validation. The provider registry will map that provider-local authorization error to the workspace setup error that tells the UI provider setup recovery is needed, not a retryable provider API outage.

Alternative considered: document all inventory failures as provider API unavailable. Rejected because invalid or revoked keys need user recovery, not retry behavior.

## Risks / Trade-offs

- Type churn across command bindings → Keep command-level request/response names stable where possible and run frontend build/lint after regenerating bindings.
- More mapper code → Keep mappers small and colocated with the boundary they adapt, and avoid generic abstraction until repeated patterns appear.
- Persistence compatibility risk → Preserve serialized Workspace payload field names unless a migration is explicitly required; add tests around stored workspace round-trips.
- Provider config relocation can create circular imports → Provider-owned config types should not import workspace command contracts; workspace/application code should depend on provider-facing profile payloads through narrow modules.
