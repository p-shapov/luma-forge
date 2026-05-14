# native-build-configuration Specification

## Purpose
TBD - created by archiving change remove-runtime-profiles. Update Purpose after archive.
## Requirements
### Requirement: Parse worker build configuration during native build

The Native build SHALL parse standardized worker configuration during the Tauri native build before producing a native binary, and SHALL expose the parsed values to native code through Cargo build environment output.

#### Scenario: Worker build configuration is available

- **WHEN** the native build receives non-empty values for Provisioner Worker image ref, Provisioner Worker port, Endpoint Worker image ref, and Endpoint Worker port
- **THEN** the build SHALL emit those values through Cargo build environment output for compile-time native use
- **AND** the app MUST NOT perform startup validation for those worker configuration values

#### Scenario: Worker build configuration is missing

- **WHEN** the native build cannot resolve any required worker configuration value from the build environment or project `.env`
- **OR** any resolved value is blank after trimming
- **THEN** the native build SHALL fail with a configuration error
- **AND** the build MUST NOT produce a usable native binary

#### Scenario: Real build environment overrides project dotenv

- **WHEN** a required worker configuration value exists both in the build environment and project `.env`
- **THEN** the build environment value SHALL take precedence

### Requirement: Defer fixed RunPod runtime values until provisioning implementation

RunPod runtime values that are not current product choices SHALL be removed from catalog/profile contracts and deferred until provisioning code introduces provider-owned implementation details.

#### Scenario: RunPod provisioning resources are created

- **WHEN** Native provisioning code needs RunPod cloud type, workspace mount path, or container disk size
- **THEN** it SHALL introduce those values inside the provider/provisioning implementation boundary
- **AND** it MUST NOT read these values from Provisioning Profile or Endpoint Profile data

