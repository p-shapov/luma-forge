## ADDED Requirements

### Requirement: Provider registry maps provisioning provider errors

The provider registry SHALL adapt provider-local provisioning failures into Workspace Provisioning use-case errors while keeping provider clients independent from Workspace Provisioning modules.

#### Scenario: Workspace Provisioning creates RunPod resources

- **WHEN** Workspace Provisioning asks the provider registry to create, observe, or delete RunPod provisioning resources
- **THEN** the provider registry SHALL call the RunPod client
- **AND** the provider registry SHALL map provider-local failures into Workspace Provisioning errors
- **AND** the RunPod client MUST NOT return Workspace Provisioning error types

#### Scenario: Provider API Key is required for provisioning

- **WHEN** Workspace Provisioning asks the provider registry to perform a RunPod provider mutation
- **THEN** the provider registry SHALL read the RunPod Provider API Key through the secret store
- **AND** the provider registry SHALL reject the operation with a Workspace Provisioning setup-prerequisite error when the key is missing or unreadable
- **AND** the provider registry MUST NOT expose the Provider API Key to Workspace Provisioning response DTOs or Workspace metadata

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
- **AND** command DTOs, Workspace metadata, logs, and diagnostics MUST NOT include the token value

#### Scenario: Provisioner token is deleted

- **WHEN** Workspace Provisioning deletes a Provisioner Worker bearer token for a Workspace
- **THEN** the secret store SHALL remove only that Workspace's provisioning token entry
- **AND** the secret store MUST NOT delete the Provider API Key entry for the GPU Cloud Provider

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
- **AND** the generated command error MUST NOT include provider transport details, Provider API Keys, Provisioner Worker bearer tokens, raw worker diagnostics, or provider request bodies
