## ADDED Requirements

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

### Requirement: Provider clients are use-case independent

Provider client implementations SHALL return provider-local results and errors instead of depending on setup, workspace setup, provisioning, or cleanup use-case error types.

#### Scenario: RunPod identity validation fails

- **WHEN** the RunPod client observes an identity transport, authorization, or response parsing failure
- **THEN** the RunPod client SHALL return a provider-local error
- **AND** the RunPod client MUST NOT return `ProviderSetupError`

#### Scenario: RunPod inventory lookup fails

- **WHEN** the RunPod client observes an inventory transport, authorization, or response parsing failure
- **THEN** the RunPod client SHALL return a provider-local error
- **AND** the RunPod client MUST NOT return `WorkspaceSetupError`

### Requirement: Provider registry maps provider errors to use-case errors

The provider registry SHALL adapt provider-local client errors into the use-case error type required by each gateway trait implementation.

#### Scenario: Provider setup validates identity

- **WHEN** Provider Setup asks the provider registry to validate identity
- **THEN** the provider registry SHALL call the provider client
- **AND** the provider registry SHALL map provider-local failures into `ProviderSetupError`

#### Scenario: Workspace Setup reads inventory

- **WHEN** Workspace Setup asks the provider registry to fetch provider inventory
- **THEN** the provider registry SHALL call the provider client
- **AND** the provider registry SHALL map provider-local failures into `WorkspaceSetupError`

### Requirement: Domain models remain provider-agnostic

Domain models SHALL remain independent from provider-specific HTTP shapes, GraphQL shapes, provider template identifiers, command handlers, Tauri runtime APIs, and secure-storage implementations.

#### Scenario: Provider-specific profile data is needed

- **WHEN** bundled profile contracts include RunPod-specific configuration
- **THEN** the provider-specific configuration SHALL live in bundled or provider boundary contracts
- **AND** generic domain profile and placement types MUST NOT depend on RunPod-specific config types

#### Scenario: Provider API response is parsed

- **WHEN** a provider module parses a provider API response
- **THEN** provider response DTOs and mapping code SHALL remain inside the provider implementation boundary
- **AND** domain modules MUST NOT import provider response DTOs

### Requirement: Workspace persistence stores provider identifiers from workspace data

Workspace catalog persistence SHALL derive persisted provider identifiers from the workspace record being stored.

#### Scenario: Workspace is inserted

- **WHEN** the Workspace Catalog inserts a Workspace record
- **THEN** the stored `gpu_cloud_provider_id` column SHALL be derived from `workspace.gpu_cloud_provider_id`
- **AND** persistence MUST NOT hardcode the v1 provider identifier

#### Scenario: Workspace is re-read after insert

- **WHEN** the Workspace Catalog re-reads a persisted Workspace record
- **THEN** the returned Workspace SHALL match the serialized Workspace payload
- **AND** the indexed provider identifier SHALL remain consistent with that payload

### Requirement: Module layout reflects native ownership boundaries

Native-layer modules SHALL be organized so that file and directory boundaries match ownership responsibilities.

#### Scenario: Workspace native code is organized

- **WHEN** workspace setup, workspace catalog, workspace contracts, and their tests are present
- **THEN** they SHALL live under the workspace module directory
- **AND** workspace test files SHALL be separate from implementation files

#### Scenario: Provider setup code is split

- **WHEN** provider setup code is split into multiple files
- **THEN** command contracts, application service orchestration, error mapping, and tests SHALL be separated by responsibility
- **AND** the split MUST NOT move provider-specific HTTP or GraphQL implementation details into provider setup

### Requirement: Existing setup behavior is preserved

The boundary refactor SHALL preserve current GPU Cloud Provider Setup and Workspace Setup behavior.

#### Scenario: GPU Cloud Provider Setup command behavior is exercised

- **WHEN** existing GPU Cloud Provider Setup tests run after the refactor
- **THEN** they SHALL continue to pass without changing the user-visible setup semantics

#### Scenario: Workspace Setup command behavior is exercised

- **WHEN** existing Workspace Setup tests run after the refactor
- **THEN** they SHALL continue to pass without changing the user-visible workspace setup semantics
