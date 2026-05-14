## MODIFIED Requirements

### Requirement: Parse worker build configuration during native build

The Native build SHALL parse worker configuration during the Tauri native build before producing a native binary, and SHALL expose the parsed values to native code through Cargo build environment output. Provisioner Worker deployment artifacts SHALL remain provider-neutral until a concrete provider requires a different provisioner artifact. Endpoint Worker deployment artifacts SHALL be provider-specific for each endpoint provider supported by the current app build.

#### Scenario: Worker build configuration is available

- **WHEN** the native build receives non-empty values for Provisioner Worker image ref, Provisioner Worker port, RunPod Endpoint Worker image ref, and RunPod Endpoint Worker port
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

#### Scenario: RunPod endpoint worker configuration is provider-qualified

- **WHEN** the native build resolves Endpoint Worker deployment configuration for RunPod
- **THEN** it SHALL read RunPod-qualified Endpoint Worker image ref and port values
- **AND** it MUST NOT require future non-RunPod providers to reuse the RunPod Endpoint Worker image ref or port values

#### Scenario: Endpoint worker configuration is not global

- **WHEN** the native build supports one or more endpoint providers
- **THEN** each supported endpoint provider SHALL have its own Endpoint Worker deployment configuration values
- **AND** the build MUST NOT expose a provider-neutral Endpoint Worker image ref as the authoritative endpoint deployment artifact
