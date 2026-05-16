## ADDED Requirements

### Requirement: Emit subprocess output to provider-visible logs

The Provisioner Worker SHALL emit output from long-running provisioning subprocesses to the worker process console so the provider's container log system can show active command progress.

#### Scenario: Dependency installation writes provider-visible logs

- **WHEN** an active provisioning job installs ComfyUI or Custom Node dependencies through a subprocess
- **THEN** the Provisioner Worker SHALL allow stdout and stderr from that subprocess to reach the worker process console
- **AND** provider pod logs SHALL be able to show dependency tool output such as package collection, download, retry, build, and error messages
- **AND** the Provisioner Worker SHALL continue enforcing cancellation and timeout behavior for that subprocess

#### Scenario: Status response remains sanitized

- **WHEN** a long-running subprocess emits output while a provisioning job is active
- **THEN** `GET /status` SHALL continue to return only the stable worker status payload
- **AND** `GET /status` MUST NOT include raw command output, request bodies, stack traces, environment dumps, provider secrets, bearer tokens, or credential-bearing URLs

#### Scenario: Subprocess failure preserves structured error contract

- **WHEN** a logged provisioning subprocess exits unsuccessfully
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL include UI-safe structured error metadata
- **AND** the worker console logs MAY include the subprocess output that explains the command failure
- **AND** the status payload MUST NOT copy raw subprocess output into the UI-safe diagnostic message
