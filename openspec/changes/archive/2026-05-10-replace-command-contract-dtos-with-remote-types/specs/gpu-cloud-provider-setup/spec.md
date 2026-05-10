## MODIFIED Requirements

### Requirement: Provider Setup services use domain-native inputs and results

Provider Setup services SHALL accept domain-native provider identifiers and secret value objects, and SHALL return domain setup snapshots instead of service-facing DTOs that duplicate command or domain models. Provider Setup command wrappers SHALL preserve generated payload compatibility while avoiding duplicated runtime command DTOs for identical nested domain snapshot shapes when command-owned remote generated binding metadata is sufficient.

#### Scenario: Setup status is read

- **WHEN** a command requests GPU Cloud Provider setup status
- **THEN** the command boundary SHALL map the generated provider id DTO to a domain `GpuCloudProviderId`
- **AND** the Provider Setup service SHALL use the domain provider id directly
- **AND** the Provider Setup service SHALL return an optional domain `GpuCloudProviderSetup`
- **AND** the command boundary SHALL expose the returned setup state through the generated command response wrapper
- **AND** the command boundary MAY use command-owned remote generated binding metadata for the nested domain setup snapshot

#### Scenario: New setup is created

- **WHEN** a command submits a generated setup request containing a provider id and Provider API Key string
- **THEN** the command boundary or Provider Setup service SHALL convert the submitted key into the domain `ProviderApiKey` value object before provider validation
- **AND** the Provider Setup service SHALL return a domain `GpuCloudProviderSetup` after validating and storing the key
- **AND** the generated command response SHALL expose only the redacted setup snapshot shape
- **AND** neither the service result nor the generated command response MUST expose the Provider API Key

#### Scenario: Existing setup is deleted

- **WHEN** a command requests deletion for a GPU Cloud Provider setup
- **THEN** the command boundary SHALL map the generated provider id DTO to a domain `GpuCloudProviderId`
- **AND** the Provider Setup service SHALL use the domain provider id directly
- **AND** the Provider Setup service SHALL return a domain-native deletion result that does not require a service-facing response DTO
- **AND** the generated command response wrapper SHALL preserve the existing `gpu_cloud_provider_setup: null` payload semantics
