## ADDED Requirements

### Requirement: Preserve worker behavior during internal package reorganization

The Provisioner Worker SHALL preserve existing runtime behavior when its internal Python modules are reorganized into responsibility-based top-level packages under `workers/provisioner/src/`.

#### Scenario: Worker API behavior remains unchanged

- **WHEN** the internal provisioner worker source layout is reorganized
- **THEN** `GET /status`, `POST /start`, and `POST /cancel` SHALL preserve their existing authorization, request validation, status, success payload, and error payload behavior
- **AND** the Provisioner Worker MUST NOT expose provider secrets, tokens, request bodies, raw command output, stack traces, environment dumps, or credential-bearing URLs because of the reorganization

#### Scenario: Preparation behavior remains unchanged

- **WHEN** the internal provisioner worker source layout is reorganized
- **THEN** successful provisioning SHALL still prepare ComfyUI, Custom Nodes, model assets, dependency records, and runtime manifest outputs according to the existing preparation contract
- **AND** failure, timeout, and cancellation cases SHALL map to the same worker job status and UI-safe error classifications as before the reorganization

#### Scenario: Module ownership is visible from package paths

- **WHEN** a developer scans the provisioner worker `src/` directory
- **THEN** HTTP adapter modules SHALL be grouped separately from orchestration modules at the top level of `src/`
- **AND** orchestration modules SHALL group job lifecycle management and high-level runtime-preparation sequencing together
- **AND** orchestration modules SHALL be grouped separately from prepared-runtime modules and auxiliary support modules
- **AND** auxiliary support modules SHALL group Git checkout, Hugging Face retrieval, filesystem path safety, and generic process execution away from application flow modules
- **AND** the runtime source tree MUST NOT require an additional `provisioner_worker` package wrapper solely to contain the worker modules
