# native-build-configuration Specification

## Purpose
TBD - created by archiving change remove-runtime-profiles. Update Purpose after archive.
## Requirements
### Requirement: Parse worker build configuration during native build
The Native build SHALL parse only non-image worker runtime configuration during the Tauri native build before producing a native binary, and SHALL expose the parsed values to native code through Cargo build environment output. Worker image refs SHALL be resolved from bundled Runtime Catalog data and Workspace-persisted runtime implementation snapshots, not from global build-time Native configuration.

#### Scenario: Worker build configuration is available
- **WHEN** the native build receives non-empty values for Provisioner Worker port and RunPod Endpoint Worker port
- **THEN** the build SHALL emit those port values through Cargo build environment output for compile-time native use
- **AND** the build MUST NOT require Provisioner Worker image ref or RunPod Endpoint Worker image ref values
- **AND** the app MUST NOT perform startup validation for worker image refs from build-time configuration

#### Scenario: Worker build configuration is missing
- **WHEN** the native build cannot resolve any required non-image worker configuration value from the build environment or project `.env`
- **OR** any resolved value is blank after trimming
- **THEN** the native build SHALL fail with a configuration error
- **AND** the build MUST NOT produce a usable native binary

#### Scenario: Real build environment overrides project dotenv
- **WHEN** a required non-image worker configuration value exists both in the build environment and project `.env`
- **THEN** the build environment value SHALL take precedence

#### Scenario: RunPod endpoint worker configuration is provider-qualified
- **WHEN** the native build resolves Endpoint Worker deployment configuration for RunPod
- **THEN** it SHALL read RunPod-qualified Endpoint Worker port values
- **AND** it MUST NOT require future non-RunPod providers to reuse the RunPod Endpoint Worker port value

#### Scenario: Worker image refs come from Runtime Catalog
- **WHEN** Workspace Setup or Workspace Provisioning needs Provisioner Worker or Endpoint Worker image refs
- **THEN** the Native Layer SHALL use the resolved runtime contract implementation snapshot selected from the bundled Runtime Catalog
- **AND** it MUST NOT use global build-time worker image refs as authoritative deployment artifacts

#### Scenario: Endpoint worker configuration is not global
- **WHEN** the native build supports one or more endpoint providers
- **THEN** each supported endpoint provider SHALL have its own non-image Endpoint Worker deployment configuration values when those values remain build-time configuration
- **AND** the build MUST NOT expose a provider-neutral Endpoint Worker image ref as the authoritative endpoint deployment artifact

### Requirement: Defer fixed RunPod runtime values until provisioning implementation

RunPod runtime values that are not current product choices SHALL be removed from catalog/profile contracts and deferred until provisioning code introduces provider-owned implementation details.

#### Scenario: RunPod provisioning resources are created

- **WHEN** Native provisioning code needs RunPod cloud type, workspace mount path, or container disk size
- **THEN** it SHALL introduce those values inside the provider/provisioning implementation boundary
- **AND** it MUST NOT read these values from Provisioning Profile or Endpoint Profile data

