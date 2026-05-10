## Context

LumaForge's native layer already separates command DTOs, application services, provider clients, secrets, and persistence. The remaining composition boundary is still thin: command handlers currently construct service dependencies directly, and workspace handlers resolve the app data directory and open/migrate the SQLite Workspace Catalog inside command execution.

This is acceptable for current read/setup commands, but Workspace Provisioning will add initiate, sync, cancel, recovery, and cleanup commands that all need consistent access to the same durable catalog and operation coordination. Keeping that wiring in handlers would duplicate infrastructure setup and make per-workspace exclusivity harder to enforce consistently.

## Goals / Non-Goals

**Goals:**

- Keep Tauri command handlers as adapters that map command DTOs, invoke application services, and map UI-safe results or errors.
- Add a managed native application state boundary for concrete infrastructure and service construction.
- Share Workspace Catalog access through native state instead of opening and migrating SQLite in individual handlers.
- Keep the existing provider setup serialization behavior while moving ownership into the shared native state.
- Leave service-level testability intact by keeping application services generic over gateway/repository traits.
- Preserve existing generated command contracts and frontend behavior.

**Non-Goals:**

- Do not introduce a general-purpose dependency injection framework.
- Do not change Workspace Setup, Provider Setup, or future Provisioning business rules.
- Do not change the SQLite schema.
- Do not change command names, request DTOs, response DTOs, or generated TypeScript bindings.
- Do not implement Workspace Provisioning as part of this change.

## Decisions

### Decision: Introduce managed native app state

Add a concrete native state object, such as `NativeAppState` or `AppServices`, and register it with Tauri through `manage(...)`. Commands receive `State<NativeAppState>` and request the service or dependency they need from that state.

Rationale:

- Tauri already supports managed state and the app already uses it for `ProviderSetupCoordinator`.
- This creates one composition root for native dependencies.
- Commands no longer need to know how to assemble catalogs, secrets, providers, repositories, and coordinators.
- Future provisioning commands can share the same operation coordination model.

Alternatives considered:

- **Factory helpers only:** smaller immediate change, but still leaves handlers responsible for composition timing and shared runtime concerns.
- **Domain-specific managed states:** useful later if provisioning grows large, but likely premature unless layered under or derived from a shared app state.
- **Full DI container:** unnecessary ceremony for the current Rust service design.

### Decision: Keep application services generic, compose them concretely in app state

Application services should continue accepting concrete dependencies through generic trait bounds. The managed state should hold concrete infrastructure values and expose methods that build concrete service instances.

Rationale:

- Existing tests can keep using fake repositories, providers, and secret stores without Tauri runtime dependencies.
- The production composition root remains explicit.
- No trait-object container is required.

### Decision: Share Workspace Catalog initialization through app state

Move app data path resolution and SQLite Workspace Catalog connection/migration out of workspace command handlers. App state should provide access to one shared catalog handle or lazily initialized catalog pool.

Rationale:

- `SqliteWorkspaceCatalog` already wraps a cloneable `SqlitePool`.
- A shared pool avoids repeated open/migrate work and gives future provisioning commands a consistent persistence boundary.
- Lazy initialization can preserve command-level `workspace_catalog_unavailable` behavior if storage initialization fails.

Alternatives considered:

- **Initialize SQLite at app startup:** simpler runtime invariant, but can turn a recoverable command-level storage problem into app launch failure.
- **Initialize SQLite per command:** current behavior; simple but duplicates setup and scales poorly into provisioning.

### Decision: Move coordinators under native app state

The existing provider setup coordinator should be owned by the native app state. Future workspace operation coordination should be added beside it, not directly into individual command modules.

Rationale:

- Serialization policy is native runtime behavior, not command DTO behavior.
- Provisioning requires at most one active sync operation per Workspace, and this concern should be consistently available to initiate, sync, cancel, and cleanup commands.

## Risks / Trade-offs

- **Risk: App state becomes a grab bag.** -> Keep it as a composition root only: store infrastructure, expose service builders, and avoid business workflow logic inside state.
- **Risk: Lazy SQLite initialization introduces concurrency edge cases.** -> Use an async-safe one-time initialization pattern or explicit mutex around catalog creation, and test concurrent command access.
- **Risk: Refactor accidentally changes command contracts.** -> Keep command DTO modules unchanged and run generated binding/build checks.
- **Risk: Provider setup serialization behavior regresses during ownership move.** -> Preserve existing coordinator semantics and cover setup/delete/create-workspace interleaving with tests.
- **Risk: Future provisioning needs richer coordination than this change adds.** -> Keep workspace operation coordination extensible, but only add the minimal primitive needed for current setup and near-term provisioning.
