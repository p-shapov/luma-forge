## ADDED Requirements

### Requirement: Command DTOs own generated binding concerns

The Native Layer SHALL keep serialization and generated frontend binding derives on command-facing DTOs rather than pure domain models.

#### Scenario: Command response exposes domain data

- **WHEN** a command returns data derived from domain models
- **THEN** the command response DTO SHALL derive the generated binding and serialization traits needed by Tauri/Specta
- **AND** the corresponding pure domain model MUST NOT be required to derive `specta::Type`

#### Scenario: Command request enters application service

- **WHEN** a command receives a generated request DTO from React
- **THEN** the command or application boundary SHALL map the DTO into application/domain input types before business validation
- **AND** domain modules MUST NOT depend on Tauri command handlers

### Requirement: Provider-specific profile config is provider-owned

Provider-specific profile configuration contracts SHALL be owned by the provider boundary that understands those fields.

#### Scenario: RunPod profile config is parsed from bundled catalogs

- **WHEN** bundled catalog data includes RunPod-specific provisioning or endpoint profile configuration
- **THEN** the RunPod-specific config structs SHALL live under the RunPod provider boundary
- **AND** the bundled catalog module MUST NOT be the owner of shared workspace profile contract types

#### Scenario: Workspace setup validates selected profiles

- **WHEN** Workspace Setup validates selected Provisioning Profile and Endpoint Profile data
- **THEN** it MAY compare provider-specific config payloads through provider-owned RunPod contract types
- **AND** it MUST NOT import RunPod-specific config types from `domain`

## MODIFIED Requirements

### Requirement: Domain models remain provider-agnostic

Domain models SHALL remain independent from provider-specific HTTP shapes, GraphQL shapes, provider template identifiers, command handlers, Tauri runtime APIs, secure-storage implementations, serialization requirements, and generated frontend binding requirements.

#### Scenario: Provider-specific profile data is needed

- **WHEN** profile contracts include RunPod-specific configuration
- **THEN** the provider-specific configuration SHALL live in provider boundary contracts
- **AND** generic domain profile and placement types MUST NOT depend on RunPod-specific config types

#### Scenario: Provider API response is parsed

- **WHEN** a provider module parses a provider API response
- **THEN** provider response DTOs and mapping code SHALL remain inside the provider implementation boundary
- **AND** domain modules MUST NOT import provider response DTOs

#### Scenario: Domain model is used in command output

- **WHEN** a domain model must be returned to React
- **THEN** the command boundary SHALL expose a command DTO mapped from the domain model
- **AND** the domain model MUST NOT derive generated frontend binding traits solely to satisfy command output requirements
