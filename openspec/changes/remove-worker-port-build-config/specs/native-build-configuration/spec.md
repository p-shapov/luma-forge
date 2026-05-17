## MODIFIED Requirements

### Requirement: Parse worker build configuration during native build
The Native build SHALL NOT parse Provisioner Worker or RunPod Endpoint Worker port or image ref values as native build-time configuration. Worker image refs SHALL be resolved from bundled Runtime Catalog data and Workspace-persisted runtime implementation snapshots, and fixed worker/provider deployment port values SHALL be owned by provisioning/provider implementation code rather than Cargo build environment output.

#### Scenario: Worker ports are absent from build environment
- **WHEN** the native build starts without `LUMA_FORGE_PROVISIONER_WORKER_PORT` or `LUMA_FORGE_RUNPOD_ENDPOINT_WORKER_PORT`
- **THEN** the native build SHALL continue without a worker port configuration error
- **AND** the build MUST NOT emit worker port values through Cargo build environment output

#### Scenario: Project dotenv omits worker ports
- **WHEN** the project `.env` omits Provisioner Worker and RunPod Endpoint Worker port values
- **THEN** the native build SHALL continue without reading those values
- **AND** the build MUST NOT require developers to maintain worker port values in local dotenv files

#### Scenario: Project dotenv omits worker image refs
- **WHEN** the project `.env` omits Provisioner Worker and RunPod Endpoint Worker image ref values
- **THEN** the native build SHALL continue without reading those values
- **AND** the build MUST NOT require developers to maintain worker image ref values in local dotenv files

#### Scenario: Worker image refs come from Runtime Catalog
- **WHEN** Workspace Setup or Workspace Provisioning needs Provisioner Worker or Endpoint Worker image refs
- **THEN** the Native Layer SHALL use the resolved runtime contract implementation snapshot selected from the bundled Runtime Catalog
- **AND** it MUST NOT use global build-time worker image refs as authoritative deployment artifacts

#### Scenario: Future native build configuration is introduced
- **WHEN** a future native build value is introduced because it is a genuine build-time choice
- **THEN** that value SHALL be parsed and validated independently from worker deployment ports
- **AND** fixed worker/provider deployment port values MUST remain outside native build-time configuration
