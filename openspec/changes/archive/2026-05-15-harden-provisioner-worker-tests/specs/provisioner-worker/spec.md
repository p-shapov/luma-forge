## ADDED Requirements

### Requirement: Cover invalid start side-effect guarantees
The Provisioner Worker test suite SHALL verify that invalid `POST /start` requests do not start preparation, mutate active job state, or write to the configured workspace.

#### Scenario: Invalid start leaves worker idle
- **WHEN** a `POST /start` request fails payload validation before a job is accepted
- **THEN** tests SHALL verify the worker status remains `idle`
- **AND** tests SHALL verify the provisioner preparation collaborator was not called
- **AND** tests SHALL verify the configured workspace has no new worker-created files or directories

#### Scenario: Invalid start rejects unsafe preset data before writes
- **WHEN** a `POST /start` request contains unsafe preset paths or identifiers
- **THEN** tests SHALL verify the request is rejected before ComfyUI checkout, Custom Node checkout, dependency installation, model download, metadata writing, or runtime manifest writing can occur

### Requirement: Cover terminal worker error mapping
The Provisioner Worker test suite SHALL verify that expected preparation failure classes map to stable UI-safe terminal job status payloads.

#### Scenario: Expected worker errors are reported through status
- **WHEN** preparation fails with a known worker error class
- **THEN** tests SHALL verify `GET /status` or the job snapshot reports status `failed`
- **AND** tests SHALL verify the terminal error payload contains the expected `code`, `reason_code`, and sanitized `message`
- **AND** tests SHALL cover Git checkout, dependency installation, model download, model authorization, path validation, and step timeout failures

#### Scenario: Unexpected preparation errors stay sanitized
- **WHEN** preparation raises an unexpected exception containing sensitive-looking text
- **THEN** tests SHALL verify the terminal status uses `unexpected_error` and `unexpected_exception`
- **AND** tests SHALL verify the original exception message and traceback are not exposed through status payloads or stderr

### Requirement: Cover symlink path escape prevention
The Provisioner Worker test suite SHALL verify that path-safety checks reject paths that resolve outside the intended workspace or prepared runtime roots through existing symlinks.

#### Scenario: Generic child path resolves through external symlink
- **WHEN** a workspace child path traverses an existing symlink that points outside the workspace root
- **THEN** tests SHALL verify the path helper rejects the resolved path before a caller can write through it

#### Scenario: Custom Node path resolves through external symlink
- **WHEN** a Custom Node checkout or requirements path resolves through an existing symlink outside the prepared ComfyUI `custom_nodes` root
- **THEN** tests SHALL verify the path helper or prepared environment validation rejects the path before checkout or dependency installation

#### Scenario: Prepared runtime paths cannot escape through symlinks
- **WHEN** runtime metadata, virtual environment, model asset, or manifest paths would resolve outside the configured workspace through a symlink
- **THEN** tests SHALL verify preparation or validation fails before reporting terminal success

### Requirement: Cover real provisioner cancellation and partial outputs
The Provisioner Worker test suite SHALL exercise cancellation against `Provisioner.prepare()` phase sequencing rather than only fake job-manager behavior.

#### Scenario: Cancellation before a preparation phase stops later work
- **WHEN** the cancellation event is set before a major preparation phase begins
- **THEN** tests SHALL verify `Provisioner.prepare()` raises cancellation
- **AND** tests SHALL verify later phase collaborators are not called
- **AND** tests SHALL verify no runtime manifest is written

#### Scenario: Cancellation during asset placement cleans partial file
- **WHEN** cancellation occurs while a model asset file is being placed into the prepared ComfyUI tree
- **THEN** tests SHALL verify the partial transfer is interrupted
- **AND** tests SHALL verify temporary partial files are removed or not promoted to the final model asset path

#### Scenario: Cancelled preparation does not report success artifacts
- **WHEN** preparation is cancelled after some workspace files have been created
- **THEN** tests SHALL verify the prepared runtime manifest is absent or invalid for success
- **AND** tests SHALL verify final validation is not treated as successful
