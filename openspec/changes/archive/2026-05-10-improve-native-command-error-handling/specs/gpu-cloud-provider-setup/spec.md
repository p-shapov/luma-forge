## ADDED Requirements

### Requirement: Provider Setup reports precise submitted-key and stored-key failures

Provider Setup commands SHALL distinguish UI-safe submitted-key failures from stored-key corruption or unreadability.

#### Scenario: Submitted Provider API Key is empty

- **WHEN** the Client submits an empty or blank Provider API Key during Provider Setup
- **THEN** the Native Layer SHALL reject the request with `provider_api_key_required`
- **AND** the Native Layer MUST NOT call the Provider
- **AND** the Native Layer MUST NOT mutate the keyring

#### Scenario: Submitted Provider API Key is unauthorized

- **WHEN** the Provider rejects the submitted Provider API Key as unauthorized, forbidden, unauthenticated, inactive, or otherwise invalid for identity validation
- **THEN** the Native Layer SHALL reject the request with `provider_api_key_unauthorized`
- **AND** the Native Layer MUST NOT mutate the keyring

#### Scenario: Stored Provider API Key is malformed

- **WHEN** a Provider Setup status, deletion, or Workspace Setup prerequisite check reads a stored keyring value that cannot be parsed as a Provider API Key
- **THEN** the Native Layer SHALL reject the read or prerequisite command with `stored_provider_api_key_invalid`
- **AND** the generated command error MUST NOT include the stored keyring value

### Requirement: Provider Setup reports precise provider identity failures

Provider Setup identity checks SHALL distinguish Provider authorization, Provider availability, and invalid Provider identity response shape.

#### Scenario: Provider identity network or availability failure

- **WHEN** RunPod identity validation fails due to timeout, DNS, connection failure, request timeout, provider outage, rate limiting, or non-auth provider availability failure
- **THEN** the Native Layer SHALL reject the command with a retryable provider availability error
- **AND** the generated command error SHALL remain UI-safe

#### Scenario: Provider identity response is malformed or incomplete

- **WHEN** RunPod identity validation receives a successful response whose data cannot produce exactly one valid active provider identity for the submitted or stored key
- **THEN** the Native Layer SHALL reject the command with `provider_identity_response_invalid`
- **AND** the generated command error MUST NOT include the Provider response body

### Requirement: Provider Setup deletion reports missing setup explicitly

Deleting local GPU Cloud Provider setup SHALL distinguish missing setup from incomplete setup prerequisites in other flows.

#### Scenario: Delete is requested when setup is missing

- **WHEN** the Client requests deletion for `runpod` and no provider keyring entry exists
- **THEN** the Native Layer SHALL reject the request with `provider_setup_not_found`
- **AND** the Native Layer MUST NOT call RunPod
- **AND** the Native Layer MUST NOT mutate local setup state

### Requirement: Provider Setup command errors guide recovery

Provider Setup command errors SHALL provide enough UI-safe information for React to guide setup recovery.

#### Scenario: Provider Setup command fails

- **WHEN** setup status, setup creation, or setup deletion fails
- **THEN** React SHALL be able to distinguish whether the user should retry, enter a different key, delete/recover local setup, refresh setup status, or resolve local keyring access
- **AND** the generated command error MUST NOT expose Provider API Keys or keyring internals
