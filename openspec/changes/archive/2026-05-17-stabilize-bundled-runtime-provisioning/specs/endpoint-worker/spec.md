## ADDED Requirements

### Requirement: Validate workspace-resolved runtime record paths
The RunPod Endpoint Worker SHALL validate prepared runtime manifest dependency record paths as mounted-workspace paths before accepting the prepared runtime environment.

#### Scenario: Dependency record paths are valid
- **WHEN** the prepared runtime manifest contains base dependency record paths that resolve under the configured workspace mount
- **AND** each referenced record file exists
- **THEN** the Endpoint Worker SHALL accept those paths as valid prepared runtime metadata

#### Scenario: Dependency record path escapes workspace
- **WHEN** the prepared runtime manifest contains a base dependency record path that resolves outside the configured workspace mount
- **THEN** the Endpoint Worker SHALL reject the prepared runtime environment with a stable UI-safe prepared runtime error
- **AND** it MUST NOT start ComfyUI

#### Scenario: Dependency record file is missing
- **WHEN** the prepared runtime manifest contains a base dependency record path under the configured workspace mount
- **AND** the referenced file does not exist
- **THEN** the Endpoint Worker SHALL reject the prepared runtime environment with a stable UI-safe missing environment error
- **AND** it MUST NOT repair the environment by installing dependencies
