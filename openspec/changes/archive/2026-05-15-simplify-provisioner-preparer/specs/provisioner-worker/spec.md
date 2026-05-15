## ADDED Requirements

### Requirement: Preserve preparation behavior during internal preparation refactors

The Provisioner Worker SHALL preserve existing preparation behavior when the internal preparation implementation is split across focused modules or services.

#### Scenario: Preparation sequence remains equivalent

- **WHEN** a valid start request is accepted and preparation succeeds
- **THEN** the Provisioner Worker SHALL clone or update ComfyUI, create or reuse the volume-local virtual environment, install ComfyUI dependencies, install declared Custom Nodes and their dependencies, download declared model assets, write dependency records, write the prepared runtime manifest, validate the prepared environment, and report terminal success according to the existing preparation contract
- **AND** the Provisioner Worker SHALL preserve the existing progress phases and terminal job status behavior

#### Scenario: Preparation failure mapping remains equivalent

- **WHEN** a Git checkout, virtual environment creation, dependency installation, public Hugging Face asset download, cancellation, timeout, or final validation failure occurs during preparation
- **THEN** the Provisioner Worker SHALL map the failure to the same UI-safe worker error class, job status, and diagnostic contract used before the internal refactor
- **AND** the response MUST NOT include provider secrets, tokens, request bodies, raw command output, stack traces, environment dumps, or credential-bearing URLs

#### Scenario: Prepared filesystem outputs remain equivalent

- **WHEN** preparation completes successfully after the internal preparation implementation is refactored
- **THEN** the mounted workspace volume SHALL contain the same required ComfyUI files, Custom Node directories, model asset files, volume-local virtual environment files, dependency records, and runtime manifest shape required by the prepared environment validation contract
- **AND** the Provisioner Worker MUST NOT write outside the validated mounted workspace paths
