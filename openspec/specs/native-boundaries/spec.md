# native-boundaries Specification

## Purpose
TBD - created by archiving change refactor-native-boundaries. Update Purpose after archive.
## Requirements
### Requirement: Command boundary owns generated command errors

The Native Layer SHALL keep command-safe error response DTOs owned by the Tauri command boundary, not by a specific application use case.

#### Scenario: Use-case error is returned from a command

- **WHEN** a native application service returns a use-case error
- **THEN** the Tauri command handler SHALL map that error into a UI-safe command error response
- **AND** the application service MUST NOT own the shared generated command error DTO

#### Scenario: Command error is exposed to React

- **WHEN** generated command bindings expose an error shape to React
- **THEN** the exposed error SHALL contain only a UI-safe code, UI-safe message, and retryability flag
- **AND** the exposed error MUST NOT include provider secrets or provider transport details

### Requirement: Provider setup recovery-required errors are explicit

The Native command boundary SHALL expose a UI-safe provider setup recovery-required error when a failed setup attempt may have left partial local setup state that could not be rolled back.

#### Scenario: Provider setup rollback fails

- **WHEN** Provider Setup reports that setup finalization failed after writing a Provider API Key and rollback deletion also failed
- **THEN** the Tauri command handler SHALL map the failure to `provider_setup_recovery_required`
- **AND** the generated command error SHALL include only a UI-safe code, UI-safe message, and retryability flag
- **AND** the generated command error MUST NOT include the submitted Provider API Key, stored Provider API Key, provider transport details, or keyring details
- **AND** the generated command error SHALL mark retrying the same setup command as not retryable

### Requirement: Provider clients are use-case independent

Provider client implementations SHALL return provider-local results and errors instead of depending on setup, workspace setup, provisioning, cleanup, or consumer-owned provider adapter error types.

#### Scenario: RunPod identity validation fails

- **WHEN** the RunPod client observes an identity transport, authorization, or response parsing failure
- **THEN** the RunPod client SHALL return a provider-local error
- **AND** the RunPod client MUST NOT return `ProviderSetupError`

#### Scenario: RunPod inventory lookup fails

- **WHEN** the RunPod client observes an inventory transport, authorization, or response parsing failure
- **THEN** the RunPod client SHALL return a provider-local error
- **AND** the RunPod client MUST NOT return `WorkspaceSetupError`

#### Scenario: RunPod client is used by consumer-owned provider adapters

- **WHEN** Provider Setup, Workspace Setup, or Workspace Resource operations call the RunPod client through their own provider adapter code
- **THEN** the RunPod client SHALL return provider-local results and errors
- **AND** the RunPod client MUST NOT depend on Provider Setup, Workspace Setup, Workspace Provisioning, Workspace Resources, or Tauri command modules

### Requirement: Provider-local errors expose stable LumaForge-owned failure variants

Provider client implementations SHALL classify provider failures into stable LumaForge-owned provider errors and SHALL keep provider-specific response interpretation inside the provider implementation boundary.

#### Scenario: RunPod REST response is classified

- **WHEN** the RunPod provider implementation receives a REST response for a provisioning resource operation
- **THEN** it SHALL classify the response into a provider-local error when the response is not successful
- **AND** `401` and `403` SHALL map to authorization failure
- **AND** `404` SHALL map to provider resource not found
- **AND** `429` SHALL map to provider rate limiting
- **AND** `409` SHALL map to provider operation conflict
- **AND** `408` and `504` SHALL map to provider operation indeterminate
- **AND** other `4xx` statuses SHALL map to provider request rejection
- **AND** other non-success statuses SHALL map to provider API unavailability
- **AND** downstream modules MUST NOT inspect RunPod-specific error codes, message strings, or response envelopes
- **AND** the provider-local failure MUST NOT include Provider API Keys, bearer headers, raw request bodies, or raw response bodies

#### Scenario: RunPod GraphQL error is classified

- **WHEN** the RunPod provider implementation receives GraphQL errors from identity or inventory requests
- **THEN** obvious authentication-related errors SHALL map to authorization failure
- **AND** other GraphQL errors SHALL map to provider request rejection
- **AND** domain and command modules MUST NOT depend on RunPod GraphQL error message strings

#### Scenario: RunPod inventory HTTP status is classified

- **WHEN** the RunPod provider implementation receives a non-success HTTP status while fetching Provider Inventory
- **THEN** authorization statuses SHALL map to authorization failure
- **AND** rate limiting SHALL map to provider rate limiting
- **AND** non-authentication `4xx` statuses SHALL map to provider request rejection
- **AND** other non-success statuses SHALL map to provider API unavailability

### Requirement: Command errors expose stable UI-safe provider recovery metadata

The Tauri command boundary SHALL map provider-related use-case errors into stable UI-safe command error metadata that reflects provider recovery semantics.

#### Scenario: Provider request is rejected

- **WHEN** a native command fails because the Provider rejected a UI-controlled request value or placement selection
- **THEN** the command error SHALL use the stable LumaForge-owned `provider_request_rejected` code or reason
- **AND** the command error SHALL mark retrying the same request as not retryable
- **AND** the recovery action SHALL guide the Client to change or reselect the invalid request value when applicable
- **AND** the command error MUST NOT expose provider-specific error codes or raw provider response details

#### Scenario: Provider is rate limited

- **WHEN** a native command fails because the Provider reports rate limiting
- **THEN** the command error SHALL use the stable LumaForge-owned `provider_rate_limited` code or reason
- **AND** the command error SHALL mark the failure as retryable when repeating the same command is safe
- **AND** the command error SHALL expose only UI-safe code, message, retryability, reason, field, and recovery action metadata

#### Scenario: Provider is unavailable

- **WHEN** a native command fails because the Provider is unavailable, timed out, or temporarily unable to complete a safe operation
- **THEN** the command error SHALL mark the failure as retryable when repeating the same command is safe
- **AND** the command error SHALL expose only UI-safe code, message, retryability, reason, field, and recovery action metadata

### Requirement: RunPod provisioning transport stays inside provider boundary

RunPod provisioning request and response shapes SHALL remain inside the RunPod provider implementation boundary.

#### Scenario: RunPod REST response is parsed

- **WHEN** the Native Layer parses RunPod REST responses for network volumes, pods, templates, or endpoints
- **THEN** provider response DTOs and mapping code SHALL remain inside `provider/runpod`
- **AND** domain modules MUST NOT import RunPod REST response DTOs
- **AND** Workspace Provisioning services MUST consume provider-neutral observations or domain snapshots instead of RunPod transport payloads

#### Scenario: RunPod serverless template metadata is persisted

- **WHEN** Workspace Provisioning persists a RunPod serverless template identifier for future cleanup
- **THEN** the persisted domain metadata SHALL represent LumaForge provider-specific provisioning state
- **AND** the persisted metadata MUST NOT contain raw RunPod HTTP request bodies, response payloads, Provider API Keys, or worker bearer tokens

### Requirement: Secret storage errors are use-case independent

Secret storage abstractions SHALL return secret-storage-owned errors instead of depending on Provider Setup, Workspace Setup, Provisioning, or Cleanup use-case error types.

#### Scenario: Provider Setup reads or writes secrets

- **WHEN** Provider Setup reads, replaces, or deletes a Provider API Key through the secret store
- **THEN** the secret store SHALL return secret-storage-owned failures
- **AND** Provider Setup SHALL map those failures into `ProviderSetupError`
- **AND** the secret store MUST NOT return `ProviderSetupError`

#### Scenario: Workspace Setup reads secrets

- **WHEN** Workspace Setup reads a Provider API Key through the secret store
- **THEN** the secret store SHALL return secret-storage-owned failures
- **AND** Workspace Setup SHALL map those failures into `WorkspaceSetupError`
- **AND** Workspace Setup MUST NOT convert from `ProviderSetupError` solely to handle secret store failures

#### Scenario: Stored Provider API Key is unreadable as a secret value

- **WHEN** the secure keyring contains a Provider API Key value that cannot be parsed as a valid Provider API Key
- **THEN** the secret store SHALL report a secret-storage-owned invalid stored key failure
- **AND** use-case mappings SHALL preserve the current UI-safe `invalid_provider_api_key` command behavior
- **AND** no command response, error, log, or metadata may include the stored secret value

### Requirement: Secret store supports per-workspace provisioning tokens

The secret store SHALL support per-workspace Provisioner Worker bearer tokens as a separate secret category from GPU Cloud Provider API Keys.

#### Scenario: Provisioner token is written

- **WHEN** Workspace Provisioning stores a Provisioner Worker bearer token for a Workspace
- **THEN** the secret store SHALL write it to a keyring scope or account that is separate from Provider API Key entries
- **AND** the secret store SHALL return secret-storage-owned failures
- **AND** the secret store MUST NOT return Workspace Provisioning error types

#### Scenario: Provisioner token is read

- **WHEN** Workspace Provisioning reads a Provisioner Worker bearer token for a Workspace
- **THEN** the secret store SHALL return the token only to native provisioning code
- **AND** command DTOs, Workspace metadata, logs, and metadata MUST NOT include the token value

#### Scenario: Provisioner token is deleted

- **WHEN** Workspace Provisioning deletes a Provisioner Worker bearer token for a Workspace
- **THEN** the secret store SHALL remove only that Workspace's provisioning token entry
- **AND** the secret store MUST NOT delete the Provider API Key entry for the GPU Cloud Provider

### Requirement: Command DTOs own generated binding concerns

The Native Layer SHALL keep generated frontend binding concerns owned by the Tauri command boundary rather than by domain models. Command-facing DTOs MAY derive generated binding traits directly, and command modules MAY provide generated binding metadata for domain models through command-owned remote type exports. Domain models MAY derive native serialization traits needed for bundled catalog parsing, local persistence, or native snapshot serialization, but they MUST NOT derive `specta::Type` or other generated frontend binding traits solely to satisfy command payload generation.

#### Scenario: Command response exposes domain data through a command wrapper

- **WHEN** a command returns data derived from domain models
- **THEN** the command response shape SHALL remain owned by the command boundary
- **AND** the command boundary MAY expose nested domain model data through command-owned remote generated binding metadata
- **AND** the corresponding domain model MUST NOT be required to derive `specta::Type`
- **AND** generated command payload field and discriminant changes SHALL be explicit in the OpenSpec change when the command contract intentionally migrates
- **AND** UI-safe error semantics SHALL remain compatible with the existing command contract

#### Scenario: Command request enters application service

- **WHEN** a command receives a generated request DTO from React
- **THEN** the command or command-adjacent mapper SHALL convert command-specific wrapper data into domain values or a service input composed of domain values before business validation
- **AND** command request DTOs MAY contain nested domain values when the command boundary owns generated binding metadata for those domain types
- **AND** domain modules MUST NOT depend on Tauri command handlers
- **AND** application services MUST NOT depend on command-owned DTO modules

#### Scenario: Provider Setup command DTOs are generated

- **WHEN** Provider Setup commands are exported as generated TypeScript bindings
- **THEN** the generated Provider Setup request and response DTOs SHALL be owned by the command boundary
- **AND** Provider Setup domain models and services MUST NOT derive `specta::Type`
- **AND** Provider Setup command names, serialized payload fields, and UI-safe error semantics SHALL remain compatible with the existing command contract

#### Scenario: Workspace Setup command DTOs are generated

- **WHEN** Workspace Setup commands are exported as generated TypeScript bindings
- **THEN** the generated Workspace Setup request and response wrappers SHALL be owned by the command boundary
- **AND** Workspace Setup command modules MAY provide command-owned remote generated binding metadata for Workspace Setup domain models
- **AND** Workspace Setup domain models and services MUST NOT derive `specta::Type`
- **AND** Workspace Setup command payload shape changes SHALL be reflected in generated TypeScript bindings and the corresponding Workspace Setup specification delta
- **AND** generated Workspace Setup bindings MUST NOT expose Provisioning Profile or Endpoint Profile command types after profiles are removed

### Requirement: Workspace Provisioning command DTOs own generated binding concerns

Workspace Provisioning command request and response DTOs SHALL be owned by the command boundary and SHALL expose generated frontend bindings without making application services depend on command DTOs.

#### Scenario: Provisioning command returns workspace and progress

- **WHEN** a Workspace Provisioning command returns data to React
- **THEN** the command response SHALL include authoritative Workspace metadata and derived Workspace Provisioning Progress
- **AND** generated binding metadata SHALL be owned by the command boundary
- **AND** Workspace Provisioning application services MUST NOT depend on command-owned DTO modules

#### Scenario: Provisioning command maps an error

- **WHEN** Workspace Provisioning returns a use-case error
- **THEN** the Tauri command handler SHALL map it into a UI-safe command error response
- **AND** the generated command error MUST NOT include provider transport details, Provider API Keys, Provisioner Worker bearer tokens, raw worker details, or provider request bodies

### Requirement: Shared provider command DTOs are not owned by Provider Setup

Generated command DTOs that are shared by multiple native flows SHALL be owned by a neutral native contract module instead of a specific application use-case module.

#### Scenario: Provider id command DTO is used by multiple flows

- **WHEN** Provider Setup, Workspace Setup, workspace persistence, or tests need the command-facing `GpuCloudProviderId`
- **THEN** they SHALL import it from a neutral shared contract module
- **AND** they MUST NOT import it from the Provider Setup module unless they are Provider Setup internals

#### Scenario: Provider id command DTO maps to domain

- **WHEN** a command-facing `GpuCloudProviderId` enters native application logic
- **THEN** it SHALL continue to map explicitly to the domain `GpuCloudProviderId`
- **AND** the domain provider id MUST NOT derive generated binding traits solely to satisfy command DTO needs

#### Scenario: Generated frontend bindings are exported

- **WHEN** generated TypeScript command bindings are exported after moving the shared provider DTO
- **THEN** `GpuCloudProviderId` SHALL remain a UI-safe generated type with the same supported v1 value, `runpod`
- **AND** command request and response payload semantics SHALL remain compatible with existing Provider Setup and Workspace Setup behavior

### Requirement: Domain models remain independent from command and provider transport boundaries

Domain models SHALL remain independent from provider-specific HTTP shapes, GraphQL response shapes, command handlers, Tauri runtime APIs, secure-storage implementations, runtime environment variable readers, and generated frontend binding requirements. Domain models MAY include provider-discriminated placement variants when those variants represent LumaForge workspace state rather than provider transport payloads.

#### Scenario: Provider-specific placement data is needed

- **WHEN** placement data includes RunPod-specific workspace placement selections
- **THEN** those selections MAY be represented by provider-discriminated domain placement variants
- **AND** domain placement types MUST NOT depend on provider HTTP or GraphQL response DTOs
- **AND** domain placement types MUST NOT contain Provisioning Profile or Endpoint Profile snapshots

#### Scenario: Provider API response is parsed

- **WHEN** a provider module parses a provider API response
- **THEN** provider response DTOs and mapping code SHALL remain inside the provider implementation boundary
- **AND** domain modules MUST NOT import provider response DTOs

#### Scenario: Domain model is used in command output

- **WHEN** a domain model must be returned to React
- **THEN** the command boundary SHALL expose a command DTO mapped from the domain model
- **AND** the domain model MUST NOT derive generated frontend binding traits solely to satisfy command output requirements

### Requirement: Workspace persistence stores provider identifiers from workspace data

Workspace catalog persistence SHALL serialize and deserialize domain Workspace records as the authoritative JSON payload, SHALL derive persisted provider identifiers from the workspace record being stored, and SHALL reject persisted Workspace rows whose indexed data is inconsistent with the serialized Workspace payload.

#### Scenario: Workspace is inserted

- **WHEN** the Workspace Catalog inserts a Workspace record
- **THEN** the stored `gpu_cloud_provider_id` column SHALL be derived from `workspace.gpu_cloud_provider_id`
- **AND** persistence MUST NOT hardcode the v1 provider identifier

#### Scenario: Workspace is re-read after insert

- **WHEN** the Workspace Catalog re-reads a persisted Workspace record
- **THEN** the returned Workspace SHALL be deserialized as a domain Workspace
- **AND** the returned Workspace SHALL match the serialized Workspace payload
- **AND** the indexed provider identifier SHALL remain consistent with that payload

#### Scenario: Workspace row data is inconsistent with payload

- **WHEN** the Workspace Catalog reads a persisted Workspace row whose indexed `id`, `name`, `gpu_cloud_provider_id`, `lifecycle_state`, or `workflow_preset_id` value disagrees with the serialized Workspace payload
- **THEN** the Workspace Catalog SHALL reject the read as unavailable
- **AND** the inconsistent Workspace MUST NOT be returned as authoritative durable state

### Requirement: Domain modules do not use broad unused-code suppressions

Native domain modules MUST NOT use broad `#[allow(dead_code)]` or `#![allow(dead_code)]` workarounds. Domain types, functions, and modules SHALL either participate in live application behavior, represent spec-defined near-term domain vocabulary with a targeted explanatory `#[allow(dead_code)]`, or be removed until live behavior requires them.

#### Scenario: Domain code is introduced

- **WHEN** a native domain type, function, or module is added
- **THEN** it SHALL be used by live native application behavior or tests that exercise live behavior
- **AND** the implementation MUST NOT suppress unused-code warnings with broad `dead_code` allowances

#### Scenario: Domain code is no longer used

- **WHEN** a native domain type, function, or module is no longer used by live native application behavior
- **THEN** it SHALL be removed or reconnected to the behavior that owns its invariant
- **AND** it MUST NOT remain in the domain as speculative placeholder code behind a broad `dead_code` allowance

#### Scenario: Spec-defined lifecycle vocabulary is ahead of implementation

- **WHEN** a native domain enum variant is part of an accepted flow specification but the implementation that constructs it has not landed yet
- **THEN** the enum MAY use a targeted `#[allow(dead_code)]`
- **AND** the allowance MUST have an adjacent comment naming the upcoming flow or behavior that will construct the currently unused vocabulary

### Requirement: Workspace lifecycle is domain-authored

Workspace lifecycle state construction and transition rules SHALL be owned by domain code. Application services MAY orchestrate prerequisites, persistence, provider calls, and command mapping, but they MUST NOT hand-author lifecycle-bearing Workspace records when a domain constructor or transition exists for that behavior.

#### Scenario: Application service creates lifecycle-bearing Workspace state

- **WHEN** an application service needs to create or change Workspace lifecycle state
- **THEN** it SHALL call a domain Workspace constructor or transition method for that lifecycle behavior
- **AND** the domain Workspace model MUST NOT depend on application services, Tauri command handlers, command DTOs, SQLite repositories, provider clients, or generated frontend binding traits

#### Scenario: Workspace state crosses a native boundary

- **WHEN** domain-authored Workspace state is persisted or returned through a command
- **THEN** the Native Layer SHALL serialize the domain Workspace directly for native persistence or map it explicitly into the command DTO for generated frontend output
- **AND** generated command payload compatibility SHALL remain owned by the command boundary

### Requirement: Application services use domain-native contracts

Native application services SHALL accept domain values or service input structs composed of domain values, and SHALL return domain results instead of service-facing DTOs that duplicate command or domain models.

#### Scenario: Workspace Setup service receives a command request

- **WHEN** a Workspace Setup command receives a generated request DTO
- **THEN** the command boundary SHALL map the request into domain values before calling the Workspace Setup service
- **AND** the Workspace Setup service MUST NOT depend on `workspace_contracts.rs` or command DTO modules

#### Scenario: Provider Setup service returns setup state

- **WHEN** Provider Setup derives setup state from provider identity
- **THEN** the Provider Setup service SHALL return a domain `GpuCloudProviderSetup`
- **AND** the command boundary SHALL map that domain setup into the generated command response DTO

### Requirement: Domain validators own domain invariants

Native domain invariants SHALL be validated by domain-owned validators grouped by the concept or aggregate being validated. Infrastructure modules MAY parse, load, and adapt errors for their boundary, but they MUST NOT become the owner of reusable domain validation rules.

#### Scenario: Bundled catalog data is parsed

- **WHEN** bundled catalog readers deserialize workflow catalogs, provisioning profiles, endpoint profiles, or provider inventory into domain values
- **THEN** bundled parsing code SHALL keep parser and reader responsibilities in the `bundled` module
- **AND** bundled parsing code SHALL delegate domain invariant checks to concept-specific domain validators such as profile, placement, workflow/catalog, or provider inventory validators
- **AND** bundled parsing code SHALL translate domain validation failures into bundled-reader errors at the infrastructure boundary

#### Scenario: Workspace Setup validates placement

- **WHEN** Workspace Setup validates a submitted provider-discriminated Placement Plan
- **THEN** placement-specific invariants SHALL be checked through a domain-owned placement validator
- **AND** profile-specific invariants SHALL be checked through a domain-owned profile validator
- **AND** Workspace Setup services MUST NOT depend on bundled catalog validator modules for reusable domain rules

### Requirement: Module layout reflects native ownership boundaries

Native-layer modules SHALL be organized so that file and directory boundaries match ownership responsibilities.

#### Scenario: Workspace native code is organized

- **WHEN** workspace setup, workspace catalog, workspace contracts, and their tests are present
- **THEN** Workspace Setup code SHALL live under the `workspace_setup` module directory
- **AND** Workspace Catalog code SHALL live under the `workspace_catalog` module directory
- **AND** workspace test files SHALL be separate from implementation files

#### Scenario: Provider setup code is split

- **WHEN** provider setup code is split into multiple files
- **THEN** command contracts, application service orchestration, error mapping, and tests SHALL be separated by responsibility
- **AND** the split MUST NOT move provider-specific HTTP or GraphQL implementation details into provider setup

### Requirement: Workspace setup and catalog have separate native module boundaries

The Native Layer SHALL keep Workspace Setup orchestration code and Workspace Catalog persistence code in separate native module directories. Workspace Setup SHALL own setup service orchestration, setup service inputs, setup errors, and setup tests. Workspace Catalog SHALL own catalog repository traits, unavailable catalog adapters, SQLite catalog persistence, and catalog persistence tests.

#### Scenario: Workspace setup module owns setup orchestration

- **WHEN** native Workspace Setup service code is compiled
- **THEN** the service, setup input contracts, setup error type, and setup-focused tests SHALL be owned by a `workspace_setup` native module directory
- **AND** production Workspace Setup imports MUST NOT use obsolete flat `crate::workspace::workspace_setup_*` module paths

#### Scenario: Workspace catalog module owns persistence

- **WHEN** native Workspace Catalog persistence code is compiled
- **THEN** the repository trait, unavailable repository adapter, SQLite implementation, and catalog persistence tests SHALL be owned by a `workspace_catalog` native module directory
- **AND** production Workspace Catalog imports MUST NOT use obsolete flat `crate::workspace::workspace_catalog_*` module paths

#### Scenario: Public behavior remains compatible

- **WHEN** commands read bundled catalogs, fetch provider inventory, read the Workspace Catalog, or create a Draft Workspace after the module split
- **THEN** command names, generated frontend payloads, UI-safe error codes, Workspace Catalog SQLite schema, and persisted Workspace semantics SHALL remain compatible with the behavior before the split

### Requirement: Native module files use role-based names

Native Rust module files SHALL use names that describe the file's local responsibility within its parent module, and SHALL NOT repeat the parent module or provider name as a filename prefix when the parent directory already provides that context.

#### Scenario: Role file is declared inside an owning module

- **WHEN** a native module directory contains secondary responsibilities such as errors, contracts, repositories, readers, parsers, mappers, handlers, or tests
- **THEN** those files SHALL be named by role, such as `error.rs`, `contracts.rs`, `repository.rs`, `reader.rs`, `parser.rs`, `mapper.rs`, `handler.rs`, or `tests.rs`
- **AND** the corresponding `mod` declarations SHALL use the same role-based names

#### Scenario: Parent context would be repeated in a filename

- **WHEN** a native file is located under an owning module directory such as `provider_setup`, `workspace_setup`, `workspace_catalog`, `bundled_catalog`, `provider`, `provider/runpod`, or a command submodule
- **THEN** the file name MUST NOT include redundant prefixes such as `provider_setup_`, `workspace_setup_`, `workspace_catalog_`, `bundled_catalog_`, `provider_client_`, `runpod_`, or `workspace_command_`

### Requirement: Bundled catalog infrastructure has an explicit module root

Bundled catalog loading infrastructure SHALL live under a native module root named `bundled_catalog` instead of a generic `bundled` module root.

#### Scenario: Bundled catalog reader is imported

- **WHEN** native code imports the bundled catalog reader, parser, errors, or tests
- **THEN** those imports SHALL use the `bundled_catalog` module root
- **AND** production code MUST NOT import bundled catalog infrastructure through `crate::bundled`

#### Scenario: Future bundled assets are introduced

- **WHEN** a future native feature introduces bundled assets that are not Workflow Catalog, Provisioning Profile, or Endpoint Profile catalog infrastructure
- **THEN** those assets SHALL NOT be forced into the `bundled_catalog` module solely because they are bundled with the application

### Requirement: Primary native module code lives in the module root

Native Rust modules with a clear primary implementation SHALL keep that primary implementation in the module's `mod.rs`, while secondary responsibilities remain in role-named sibling files.

#### Scenario: Module has a central service or client

- **WHEN** a native module's primary responsibility is represented by a central service, coordinator-facing service API, provider client, or command handler set
- **THEN** that primary implementation SHALL live in the module's `mod.rs`
- **AND** sibling files SHALL be reserved for secondary roles such as errors, contracts, persistence adapters, mappers, and tests

#### Scenario: Module public surface is imported by other boundaries

- **WHEN** another native boundary imports the primary type or behavior of a module
- **THEN** the import SHALL be available from the owning module root
- **AND** imports of secondary details SHALL use the role-based child module path when those details are intentionally exposed

### Requirement: Native module layout refactors preserve behavior

Native module layout standardization SHALL preserve existing runtime behavior, generated command contracts, persisted state semantics, provider behavior, and UI-safe error semantics.

#### Scenario: Native module files are renamed

- **WHEN** native Rust files are renamed or primary implementations are moved into `mod.rs`
- **THEN** command names, generated TypeScript payloads, SQLite schema, domain validation behavior, provider request and response behavior, and UI-safe command error mappings SHALL remain compatible with the behavior before the refactor

#### Scenario: Native verification is run

- **WHEN** the module layout refactor is complete
- **THEN** `cargo test` SHALL pass
- **AND** `cargo clippy --fix --allow-dirty --allow-staged` SHALL complete successfully
- **AND** `cargo fmt` SHALL complete successfully

### Requirement: Existing setup behavior is preserved

The boundary refactor SHALL preserve current GPU Cloud Provider Setup and Workspace Setup behavior.

#### Scenario: GPU Cloud Provider Setup command behavior is exercised

- **WHEN** existing GPU Cloud Provider Setup tests run after the refactor
- **THEN** they SHALL continue to pass without changing the user-visible setup semantics

#### Scenario: Workspace Setup command behavior is exercised

- **WHEN** existing Workspace Setup tests run after the refactor
- **THEN** they SHALL continue to pass without changing the user-visible workspace setup semantics

### Requirement: Native app state owns service wiring

The Native Layer SHALL own production dependency composition in managed native application state rather than in individual Tauri command handlers.

#### Scenario: Command invokes a native application service

- **WHEN** a Tauri command handler receives a command request
- **THEN** the handler SHALL map the request into native service input before invoking the application service
- **AND** the handler SHALL obtain production service dependencies from managed native application state
- **AND** the handler MUST NOT construct provider clients, secret stores, bundled catalog readers, or workspace repositories directly as part of command-specific business flow wiring

#### Scenario: Native app starts

- **WHEN** the Tauri app initializes native runtime state
- **THEN** the Native Layer SHALL register managed state that can provide production services for Provider Setup and Workspace Setup commands
- **AND** the managed state SHALL keep business workflow decisions inside application services rather than inside the state object

### Requirement: Workspace catalog runtime is shared outside handlers

The Native Layer SHALL manage Workspace Catalog path resolution, SQLite connection, and migration through native application state instead of repeating that runtime setup in workspace command handlers.

#### Scenario: Workspace command needs catalog access

- **WHEN** a Workspace Setup command needs to read or write the local Workspace Catalog
- **THEN** the command SHALL obtain catalog access through managed native application state
- **AND** the command handler MUST NOT resolve the app data directory or open and migrate the SQLite Workspace Catalog directly

#### Scenario: Workspace catalog initialization fails

- **WHEN** managed native application state cannot initialize or access the SQLite Workspace Catalog for a command
- **THEN** the command SHALL fail with the existing UI-safe Workspace Catalog or local storage error semantics
- **AND** the command MUST NOT return partial Workspace Catalog data as authoritative

#### Scenario: Multiple commands access the Workspace Catalog

- **WHEN** multiple native commands access the Workspace Catalog during one app runtime
- **THEN** the Native Layer SHALL reuse managed catalog access after successful initialization
- **AND** repeated command handling MUST NOT perform independent SQLite connection and migration setup for each command invocation

### Requirement: Operation coordinators are native runtime state

The Native Layer SHALL keep cross-command operation coordinators in managed native application state rather than constructing or owning them inside command modules.

#### Scenario: Provider setup operation is serialized

- **WHEN** Provider Setup, Provider Setup deletion, or Workspace creation needs provider setup serialization
- **THEN** the command SHALL acquire the provider setup operation guard through managed native application state
- **AND** serialization behavior SHALL remain consistent with the existing provider setup coordinator semantics

#### Scenario: Workspace operation coordination is introduced

- **WHEN** a native workflow needs per-Workspace operation serialization
- **THEN** the coordinator SHALL be owned by managed native application state
- **AND** commands participating in that workflow SHALL use the shared coordinator instead of defining command-local locks

### Requirement: Command errors expose actionable UI-safe categories

The Native command boundary SHALL expose command errors with stable UI-safe categories that are specific enough for React to render recovery guidance without parsing human-readable messages.

#### Scenario: Command error is returned to React

- **WHEN** a native command fails with a known use-case, provider, storage, catalog, validation, or persistence failure
- **THEN** the generated command error SHALL include a stable `code`
- **AND** the generated command error SHALL include a UI-safe `message`
- **AND** the generated command error SHALL include a `retryable` flag
- **AND** the generated command error MAY include optional UI-safe metadata such as affected field, reason category, or recovery action
- **AND** the generated command error MUST NOT include Provider API Keys, keyring values, raw provider transport bodies, raw GraphQL responses, raw SQLite errors, raw SQL statements, raw `workspace_json`, stack traces, or implementation-only source errors

#### Scenario: Frontend handles command error

- **WHEN** React receives a generated native command error
- **THEN** React SHALL be able to choose user-facing copy and recovery affordances from the error `code` and optional safe metadata
- **AND** React MUST NOT parse `message` to determine recovery behavior

### Requirement: Command error contract is generated and reference-aligned

The Native command error contract SHALL remain generated from the Tauri command boundary and reflected in reference contract documentation.

#### Scenario: Command error codes change

- **WHEN** a native command error code or error metadata field is added, removed, or renamed
- **THEN** generated TypeScript command bindings SHALL be regenerated
- **AND** the reference native contract SHALL be updated to match the generated contract
- **AND** command mapping tests SHALL prove every native use-case error maps to a UI-safe command error

### Requirement: Command boundary preserves precise internal categories until UI-safe mapping

Native implementation boundaries SHALL preserve non-secret failure categories internally until they can be mapped into a UI-safe command error.

#### Scenario: Low-level dependency fails

- **WHEN** provider transport, keyring access, bundled catalog parsing, Workspace Catalog persistence, or request validation fails
- **THEN** the owning native boundary SHALL represent the failure with a typed non-secret category before command mapping
- **AND** immediate mapping to a broad command error SHALL occur only when the detailed category is intentionally not actionable

#### Scenario: Internal error crosses command boundary

- **WHEN** an internal error reaches a Tauri command handler
- **THEN** the command handler or command-adjacent mapper SHALL convert it into the generated command error contract
- **AND** the application service MUST NOT return the command-owned DTO directly

### Requirement: Workspace Command DTOs Exclude RunPod Template Runtime Environment
The Native command boundary SHALL expose RunPod endpoint template metadata to React only through a UI-safe shape that excludes provider-returned runtime environment values.

#### Scenario: Workspace response includes RunPod endpoint template metadata
- **WHEN** a generated Workspace command response includes a RunPod endpoint template snapshot
- **THEN** the generated binding-safe DTO SHALL include only UI-safe template metadata needed by the client
- **AND** the DTO MUST NOT include runtime environment keys, runtime environment values, Provider API Keys, worker bearer tokens, provider-owned env values, or operator-added template env values

#### Scenario: Generated bindings are exported
- **WHEN** generated TypeScript command bindings are exported for Workspace payloads
- **THEN** the exported RunPod endpoint template snapshot type SHALL NOT contain a `runtime_env` field
- **AND** React MUST NOT depend on endpoint template environment maps for provisioning state, cleanup state, or readiness state

#### Scenario: Legacy Workspace metadata is mapped to a command response
- **WHEN** the command boundary maps a Workspace loaded from legacy metadata that included RunPod template runtime environment values
- **THEN** the command response SHALL omit those runtime environment values
- **AND** no command response, command error, log, or metadata SHALL expose the legacy values

### Requirement: Provider resource contracts are use-case independent

Provider resource contracts used to create, discover, observe, and delete GPU provider resources SHALL be owned by the Workspace Resource boundary rather than by the Workspace Provisioning use case.

#### Scenario: Workspace Provisioning mutates provider resources

- **WHEN** Workspace Provisioning needs to create, observe, discover, or delete provider resources
- **THEN** it SHALL depend on provider-neutral resource contracts and gateway traits outside the Workspace Provisioning module
- **AND** Workspace Provisioning SHALL remain responsible for provisioning phase decisions, durable Workspace updates, and provisioning failure semantics
- **AND** the provider resource boundary MUST NOT expose Provider API Keys, worker bearer tokens, raw RunPod transport payloads, or command DTOs

#### Scenario: Workspace Resource Cleanup deletes provider resources

- **WHEN** Workspace Resource Cleanup deletes known provider resources for a Workspace
- **THEN** it SHALL depend on provider-neutral resource gateway traits outside the Workspace Provisioning module
- **AND** it MUST NOT import Workspace Provisioning modules solely to access provider resource CRUD abstractions
- **AND** cleanup SHALL preserve its existing tolerance for provider resources that are already missing

#### Scenario: Workspace Resource operations implement provider resource access

- **WHEN** Workspace Resource-owned adapters perform RunPod resource operations for native use cases
- **THEN** they SHALL implement the provider-neutral resource gateway without depending on Workspace Provisioning modules
- **AND** they SHALL keep RunPod request construction, provider resource naming, fixed RunPod port values, secret-backed Provider API Key lookup, and provider-local error mapping inside the Workspace Resource boundary
- **AND** RunPod request and response DTO parsing SHALL remain inside the RunPod provider client boundary

### Requirement: Workspace Provisioning service remains an orchestration boundary

The Workspace Provisioning application service SHALL orchestrate the provisioning lifecycle through smaller phase-specific components while preserving the externally visible Workspace Provisioning behavior.

#### Scenario: Provisioning sync is refactored into phase modules

- **WHEN** Workspace Provisioning sync implementation is split across internal modules
- **THEN** the Native Layer SHALL preserve the existing phase order for network volume, provisioning pod, environment preparation, provisioning pod termination, endpoint template, serverless endpoint, and readiness validation
- **AND** each sync call SHALL continue to perform at most one provider, worker, or catalog mutation before returning authoritative Workspace metadata and derived progress
- **AND** concurrent sync behavior for the same Workspace SHALL remain read-only for the overlapping request

#### Scenario: Domain state helpers are extracted

- **WHEN** reusable Workspace lifecycle, readiness, failure-detail, cleanup-reset, or progress-derivation logic is moved out of the provisioning service
- **THEN** extracted helpers SHALL preserve existing Workspace lifecycle states, failure codes, failure sources, recovery actions, progress phases, and persisted snapshot shapes
- **AND** extracted helpers MUST NOT introduce dependencies from domain modules to Tauri commands, provider transport DTOs, secret storage implementations, or generated frontend binding concerns

#### Scenario: Command contract remains stable

- **WHEN** Workspace Provisioning internals are reorganized
- **THEN** the Tauri Workspace Provisioning commands SHALL keep their existing request and response payload shape
- **AND** generated TypeScript binding compatibility SHALL be preserved unless a separate spec change explicitly changes the command contract
- **AND** no Provider API Key, Provisioner Worker bearer token, raw provider payload, or provider transport detail may be added to command responses, Workspace metadata, logs, or metadata

### Requirement: Consumer-owned provider adapters map provider errors to use-case errors

Consumer-owned provider adapters SHALL adapt provider-local client errors into the use-case error type required by each gateway trait implementation.

#### Scenario: Provider setup validates identity

- **WHEN** Provider Setup asks its provider identity gateway to validate identity
- **THEN** the Provider Setup-owned provider adapter SHALL call the provider client
- **AND** the Provider Setup-owned provider adapter SHALL map provider-local failures into `ProviderSetupError`
- **AND** the shared provider package MUST NOT own this Provider Setup error mapping

#### Scenario: Workspace Setup reads inventory

- **WHEN** Workspace Setup asks its provider placement gateway to fetch provider inventory
- **THEN** the Workspace Setup-owned provider adapter SHALL call the provider client
- **AND** the Workspace Setup-owned provider adapter SHALL map provider-local failures into `WorkspaceSetupError`
- **AND** the shared provider package MUST NOT own this Workspace Setup error mapping

### Requirement: Consumer-owned resource operations map provisioning provider errors

Workspace Resource operation adapters SHALL adapt provider-local provisioning resource failures into workspace-resource boundary errors while keeping provider clients and workspace resource contracts independent from Workspace Provisioning modules.

#### Scenario: Workspace Provisioning creates RunPod resources

- **WHEN** Workspace Provisioning asks Workspace Resource operations to create, observe, or delete RunPod provisioning resources
- **THEN** the Workspace Resource-owned RunPod operation adapter SHALL call the RunPod client
- **AND** the Workspace Resource-owned RunPod operation adapter SHALL map provider-local failures into workspace-resource boundary errors
- **AND** Workspace Provisioning SHALL map workspace-resource boundary errors into Workspace Provisioning errors at the use-case boundary
- **AND** the RunPod client MUST NOT return Workspace Provisioning error types
- **AND** the workspace resource boundary MUST NOT depend on Workspace Provisioning modules

#### Scenario: Provider API Key is required for provisioning

- **WHEN** Workspace Provisioning asks Workspace Resource operations to perform a RunPod provider mutation
- **THEN** the Workspace Resource-owned RunPod operation adapter SHALL read the RunPod Provider API Key through the secret store
- **AND** the Workspace Resource-owned RunPod operation adapter SHALL reject the operation with a workspace-resource setup-prerequisite error when the key is missing or unreadable
- **AND** Workspace Provisioning SHALL map that workspace-resource setup-prerequisite error to the existing Workspace Provisioning setup-prerequisite error
- **AND** the Workspace Resource-owned RunPod operation adapter MUST NOT expose the Provider API Key to Workspace Provisioning response DTOs or Workspace metadata

### Requirement: Consumer-owned provider adapters map provider errors to stable recovery semantics

Consumer-owned provider adapters SHALL map provider-local errors into boundary-appropriate errors for Provider Setup, Workspace Setup, and Workspace Resource access without leaking provider transport details.

#### Scenario: Provider setup and workspace setup map provider errors

- **WHEN** Provider Setup or Workspace Setup receives a provider-local failure through its own provider adapter
- **THEN** the adapter SHALL map the provider-local error into the corresponding use-case error
- **AND** unauthorized provider keys SHALL remain non-retryable setup recovery failures
- **AND** provider API unavailability and rate limiting SHALL remain retryable provider availability failures where the use case exposes a retryable availability category
- **AND** provider request rejection SHALL remain distinct from provider API unavailability
- **AND** the shared provider package MUST NOT map provider-local errors into Provider Setup or Workspace Setup error types

#### Scenario: Workspace Provisioning maps workspace resource errors

- **WHEN** Workspace Provisioning receives a workspace-resource boundary failure through Workspace Resource operations
- **THEN** Workspace Provisioning SHALL map rate limiting to a provider rate-limited provisioning error
- **AND** Workspace Provisioning SHALL map request rejection to a provider request-rejected provisioning error
- **AND** Workspace Provisioning SHALL preserve existing provider setup, authorization, availability, invalid response, not found, conflict, and indeterminate operation semantics
- **AND** the mapped error MUST NOT expose provider transport details, Provider API Keys, bearer headers, raw provider payloads, or provider-specific error codes as domain contracts

### Requirement: Workspace provisioner owns environment orchestration
Workspace environment preparation orchestration SHALL live in a native workspace provisioner boundary that is separate from provider-resource operations and encapsulates the low-level Provisioner Worker protocol gateway.

#### Scenario: Provider resources remain outside workspace provisioner
- **WHEN** native code needs to create, discover, observe, or delete RunPod volumes, provisioning pods, endpoint templates, or serverless endpoints
- **THEN** it SHALL use the Workspace Resource boundary
- **AND** the workspace provisioner MUST NOT contain RunPod request or response DTOs
- **AND** the workspace provisioner MUST NOT create, discover, observe, or delete provider resources

#### Scenario: Worker protocol is encapsulated by workspace provisioner
- **WHEN** native code needs to call the Provisioner Worker HTTP API or use Provisioner Worker status types
- **THEN** it SHALL depend on the workspace provisioner boundary
- **AND** the low-level Provisioner Worker protocol module SHALL be nested under the workspace provisioner boundary as `gateway`
- **AND** crate-root native modules MUST NOT import a sibling `provisioner_worker` module

#### Scenario: Workspace provisioning remains top-level coordinator
- **WHEN** a Workspace Provisioning sync command selects the next safe provisioning activity
- **THEN** Workspace Provisioning SHALL remain responsible for loading authoritative Workspace metadata, enforcing per-workspace sync coordination, sequencing provider-resource steps, and returning command-safe results
- **AND** it MAY delegate environment preparation to the workspace provisioner
- **AND** the workspace provisioner MUST NOT own the full Draft-to-Ready Workspace lifecycle
