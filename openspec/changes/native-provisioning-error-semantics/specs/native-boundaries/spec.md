## ADDED Requirements

### Requirement: Command mapping preserves provisioning error semantics

The Tauri command boundary SHALL map `WorkspaceProvisioningError` and related setup/resource categories into stable UI-safe `NativeCommandError` metadata without exposing low-level implementation details.

#### Scenario: Immediate provisioning command error is mapped

- **WHEN** Workspace Provisioning returns an immediate command failure for workspace identity, lifecycle state, catalog/persistence, secret/keyring, conflict, transient provider availability, transient worker availability, or escaped resource-operation failure
- **THEN** the command boundary SHALL return a stable `NativeCommandErrorCode`, message, retryability flag, reason, and recovery action aligned with that category
- **AND** the command error MUST NOT expose SQLite errors, migration SQL, raw filesystem details, reqwest details, keyring details, RunPod-specific errors, raw request bodies, raw response bodies, Provider API Keys, or Provisioner Worker bearer tokens

#### Scenario: Persisted provisioning failure is returned through workspace payload

- **WHEN** Workspace Provisioning persists a structured `WorkspaceProvisioningFailure`
- **THEN** command responses SHALL expose that failure through generated binding-safe Workspace or progress payload fields
- **AND** the command boundary MUST NOT replace the persisted recovery-required failure with a generic command error when the command can return the authoritative Workspace state

#### Scenario: Catalog command mappings are granular

- **WHEN** command mapping receives Workspace Catalog storage unavailable, migration failed, query failed, corrupt data, schema mismatch, or generic unavailable categories
- **THEN** each category SHALL map to stable UI-safe command metadata
- **AND** categories MUST NOT collapse into generic Workspace Catalog unavailable behavior when the specific category is known

#### Scenario: Command mapping is regression tested

- **WHEN** regression tests cover `WorkspaceProvisioningError -> NativeCommandError` mappings
- **THEN** each tested category SHALL assert code, reason, retryability, and recovery action
- **AND** tests SHALL assert that mapped command errors remain implementation-safe
