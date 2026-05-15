## ADDED Requirements

### Requirement: Preserve Workspace Provisioning Behavior During Native Refactor
The Native Layer SHALL preserve the existing Workspace Provisioning command contract, durable sync semantics, cleanup metadata guarantees, and secret-safety behavior when the native provisioning implementation is split into focused Rust modules.

#### Scenario: Refactored sync preserves single-action semantics
- **WHEN** the Client syncs a Workspace whose lifecycle state is `provisioning`
- **THEN** the Native Layer SHALL continue to derive the next safe provisioning activity from authoritative Workspace metadata
- **AND** the Native Layer SHALL perform at most one provider, worker, or catalog mutation activity for that sync call
- **AND** the Native Layer SHALL persist resulting Workspace metadata before reporting success for the activity

#### Scenario: Refactored module preserves command contract
- **WHEN** the Client initiates, syncs, or cancels Workspace Provisioning through existing Tauri commands
- **THEN** the Native Layer SHALL return the same UI-safe response shapes and error categories as before the refactor
- **AND** generated frontend bindings MUST NOT require a frontend import or contract change because of the native module split

#### Scenario: Refactored implementation preserves cleanup metadata
- **WHEN** a provisioning action fails after any provider resource identifier is known
- **THEN** the Native Layer SHALL retain the known Workspace resource snapshots required for future cleanup
- **AND** the Native Layer MUST NOT clear provisioning snapshots except through the existing successful cancellation cleanup policy

#### Scenario: Refactored implementation preserves secret safety
- **WHEN** Workspace Provisioning reads Provider API Keys or Provisioner Worker bearer tokens during initiate, sync, or cancel
- **THEN** the Native Layer SHALL keep those secrets behind secure storage and provider or worker call paths
- **AND** command responses, Workspace metadata, logs, diagnostics, and generated frontend bindings MUST NOT expose those secrets
