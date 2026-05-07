## ADDED Requirements

### Requirement: Native reports redacted provider setup status

The Native Layer SHALL expose a local-only command that returns the current GPU Cloud Provider Setup status using only UI-safe data. The response SHALL include complete setup metadata only when the selected GPU Cloud Provider metadata is present in SQLite and the matching Provider API Key is present in the secure keyring.

#### Scenario: Complete setup exists locally

- **WHEN** Client requests GPU Cloud Provider Setup status and SQLite contains provider setup metadata with a matching Provider API Key present in keyring
- **THEN** Native Layer returns redacted setup status containing GPU Cloud Provider identity, provider user id, and provider API key fingerprint

#### Scenario: No setup exists

- **WHEN** Client requests GPU Cloud Provider Setup status and neither SQLite setup metadata nor a matching keyring Provider API Key exists
- **THEN** Native Layer returns no completed setup

#### Scenario: Key exists but metadata is missing

- **WHEN** Client requests GPU Cloud Provider Setup status and keyring contains a Provider API Key but SQLite setup metadata is missing
- **THEN** Native Layer returns no completed setup without calling the Provider and without persisting setup metadata

#### Scenario: Metadata exists but key is missing

- **WHEN** Client requests GPU Cloud Provider Setup status and SQLite contains provider setup metadata but the matching Provider API Key is missing from keyring
- **THEN** Native Layer rejects the status request as incomplete local provider setup and returns no Provider API Key secret

### Requirement: Native validates and stores one provider setup

The Native Layer SHALL accept one setup request containing a supported GPU Cloud Provider id and Provider API Key, validate the key through the selected GPU Provider implementation, store the Provider API Key in the secure keyring, persist redacted provider setup metadata in SQLite, re-read local state, and return only redacted setup status.

#### Scenario: Successful RunPod setup

- **WHEN** Client submits `runpod` with a non-empty Provider API Key that validates through the RunPod provider implementation
- **THEN** Native Layer stores the Provider API Key in secure keyring, persists provider setup metadata in SQLite, and returns redacted setup status without the Provider API Key

#### Scenario: Unsupported provider

- **WHEN** Client submits setup for an unsupported GPU Cloud Provider id
- **THEN** Native Layer rejects the request before provider validation, keyring mutation, or SQLite mutation

#### Scenario: Empty provider key

- **WHEN** Client submits setup with an empty Provider API Key
- **THEN** Native Layer rejects the request before provider validation, keyring mutation, or SQLite mutation

#### Scenario: Existing complete setup

- **WHEN** Client submits setup after complete GPU Cloud Provider Setup already exists
- **THEN** Native Layer rejects the request without provider validation, keyring mutation, or SQLite mutation

### Requirement: RunPod key validation derives redacted identity

The RunPod provider implementation SHALL validate a submitted Provider API Key by performing an identity-focused provider request, deriving provider user id from the provider user identity, and deriving provider API key fingerprint from the matching active provider API key id. Setup SHALL NOT require provider permission enforcement beyond this identity and active-key validation.

#### Scenario: Active key identity matches

- **WHEN** RunPod accepts the submitted Provider API Key and returns a provider user identity with an active API key id that matches the submitted key
- **THEN** Native Layer treats the key as valid for setup and uses the provider user id and API key id as redacted setup metadata

#### Scenario: Provider rejects key

- **WHEN** RunPod rejects the submitted Provider API Key or does not return provider user identity
- **THEN** Native Layer rejects setup as invalid provider credentials without keyring mutation or SQLite mutation

#### Scenario: No active matching key

- **WHEN** RunPod accepts the submitted Provider API Key but no active returned API key id matches the submitted key
- **THEN** Native Layer rejects setup as invalid provider credentials without keyring mutation or SQLite mutation

#### Scenario: Insufficient permissions are discovered later

- **WHEN** the submitted Provider API Key validates identity during setup but lacks permissions required by a later Workspace Setup or Workspace Provisioning operation
- **THEN** the later Native-owned flow handles the provider permission failure at its operation boundary without changing the completed setup contract

### Requirement: Provider setup mutations fail closed

The Native Layer SHALL report setup success only after Provider API Key storage and SQLite setup metadata persistence both succeed and local setup state is re-read as complete. Partial setup SHALL NOT be reported as successful.

#### Scenario: Keyring write fails

- **WHEN** provider key validation succeeds but Native Layer cannot store the Provider API Key in secure keyring
- **THEN** Native Layer rejects setup and does not persist provider setup metadata in SQLite

#### Scenario: SQLite write fails after keyring write

- **WHEN** Native Layer stores the Provider API Key in keyring but cannot persist provider setup metadata in SQLite
- **THEN** Native Layer attempts to delete the newly written keyring entry, rejects setup, and does not report completed setup

#### Scenario: Redacted status reread fails

- **WHEN** Native Layer stores the Provider API Key and persists provider setup metadata but cannot re-read local setup state as complete
- **THEN** Native Layer rejects setup and does not return the submitted Provider API Key

### Requirement: Provider setup is idempotent by completed provider

The Native Layer SHALL reject any setup submission after complete GPU Cloud Provider Setup exists, regardless of whether the submitted Provider API Key matches the existing key.

#### Scenario: Retried setup after observed success

- **WHEN** Client retries setup after Native Layer has already completed and persisted GPU Cloud Provider Setup
- **THEN** Native Layer rejects the retry as existing setup and does not mutate keyring or SQLite provider setup metadata

#### Scenario: Concurrent setup submissions

- **WHEN** multiple setup submissions are received while no complete setup exists
- **THEN** Native Layer serializes setup mutation so at most one request can complete and all later requests observe existing setup or a failed setup state

### Requirement: Native syncs provider setup from existing provider key

The Native Layer SHALL expose an explicit sync command that reads an existing Provider API Key from keyring, validates it through the GPU Provider abstraction, persists refreshed redacted setup metadata in SQLite, and returns redacted setup status only after validation succeeds.

#### Scenario: Existing key can be synced

- **WHEN** Client requests GPU Cloud Provider Setup sync and keyring contains a RunPod Provider API Key
- **THEN** Native Layer validates the key through the RunPod provider implementation, persists refreshed redacted setup metadata in SQLite, and returns completed redacted setup status

#### Scenario: Existing key cannot be synced

- **WHEN** Client requests GPU Cloud Provider Setup sync and keyring contains a Provider API Key that cannot be validated through the provider implementation
- **THEN** Native Layer rejects or reports setup unavailable without exposing the Provider API Key and without persisting completed setup metadata

#### Scenario: No key exists during sync

- **WHEN** Client requests GPU Cloud Provider Setup sync and keyring has no Provider API Key for the selected provider
- **THEN** Native Layer rejects sync as incomplete provider setup without calling the Provider and without persisting setup metadata

### Requirement: Native deletes local provider setup

The Native Layer SHALL expose a command that deletes local GPU Cloud Provider Setup for the selected provider by removing the Provider API Key from secure keyring and removing redacted setup metadata from SQLite. Deletion SHALL NOT call the Provider and SHALL NOT delete Provider Resources.

#### Scenario: Existing setup is deleted

- **WHEN** Client requests GPU Cloud Provider Setup deletion and local setup metadata or a keyring Provider API Key exists for the selected provider
- **THEN** Native Layer deletes the Provider API Key from keyring, deletes setup metadata from SQLite, and returns no completed setup

#### Scenario: Delete is retried after setup is absent

- **WHEN** Client requests GPU Cloud Provider Setup deletion and no setup metadata or keyring Provider API Key exists for the selected provider
- **THEN** Native Layer treats deletion as complete and returns no completed setup

#### Scenario: Keyring deletion fails

- **WHEN** Client requests GPU Cloud Provider Setup deletion and Native Layer cannot delete or verify absence of the Provider API Key from keyring
- **THEN** Native Layer rejects deletion without deleting setup metadata from SQLite

#### Scenario: Metadata deletion fails after key deletion

- **WHEN** Client requests GPU Cloud Provider Setup deletion and Native Layer deletes the keyring Provider API Key but cannot delete SQLite setup metadata
- **THEN** Native Layer rejects deletion and subsequent setup status is not reported as complete

### Requirement: Provider API Key remains secret

The Native Layer SHALL ensure Provider API Keys are never returned to React, persisted in SQLite, included in generated frontend types as response fields, or included in logs, diagnostics, errors, or domain snapshots.

#### Scenario: Setup response redacts secret

- **WHEN** setup succeeds or fails
- **THEN** Native Layer response and errors contain no Provider API Key material beyond the provider API key fingerprint

#### Scenario: Status response redacts secret

- **WHEN** Client requests setup status
- **THEN** Native Layer response contains no Provider API Key secret and only includes redacted setup metadata when setup is complete
