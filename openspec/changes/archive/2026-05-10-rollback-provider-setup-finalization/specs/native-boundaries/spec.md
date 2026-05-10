## ADDED Requirements

### Requirement: Provider setup recovery-required errors are explicit

The Native command boundary SHALL expose a UI-safe provider setup recovery-required error when a failed setup attempt may have left partial local setup state that could not be rolled back.

#### Scenario: Provider setup rollback fails

- **WHEN** Provider Setup reports that setup finalization failed after writing a Provider API Key and rollback deletion also failed
- **THEN** the Tauri command handler SHALL map the failure to `provider_setup_recovery_required`
- **AND** the generated command error SHALL include only a UI-safe code, UI-safe message, and retryability flag
- **AND** the generated command error MUST NOT include the submitted Provider API Key, stored Provider API Key, provider transport details, or keyring diagnostics
- **AND** the generated command error SHALL mark retrying the same setup command as not retryable
