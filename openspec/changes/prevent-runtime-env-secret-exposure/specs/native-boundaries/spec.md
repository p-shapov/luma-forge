## ADDED Requirements

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
- **AND** no command response, command error, log, or diagnostic SHALL expose the legacy values
