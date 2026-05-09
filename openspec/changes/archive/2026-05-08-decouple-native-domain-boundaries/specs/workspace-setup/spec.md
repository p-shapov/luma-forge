## MODIFIED Requirements

### Requirement: Read provider placement inventory

The Native Layer SHALL expose a command that returns placement inventory for an explicit GPU Cloud Provider after validating local provider setup prerequisites.

#### Scenario: Provider setup is complete

- **WHEN** the Client requests provider inventory for `runpod` and the local Provider API Key exists
- **THEN** the Native Layer SHALL call RunPod using the stored Provider API Key
- **AND** the Native Layer SHALL return available datacenters, GPU options per datacenter, and provider maximum Persistent Storage Volume size when known
- **AND** the response MUST NOT include the Provider API Key

#### Scenario: Provider setup is incomplete

- **WHEN** the Client requests provider inventory and the required local Provider API Key is missing
- **THEN** the Native Layer SHALL reject the request with `provider_setup_incomplete`
- **AND** the Native Layer MUST reject before calling the Provider

#### Scenario: Provider API Key is invalid or revoked

- **WHEN** the Client requests provider inventory and the Provider rejects the stored Provider API Key as unauthorized or forbidden
- **THEN** the Native Layer SHALL reject the request with `invalid_provider_api_key`
- **AND** the Native Layer MUST NOT report the failure as retryable
- **AND** the Native Layer MUST NOT mutate the Workspace Catalog

#### Scenario: Provider inventory lookup fails

- **WHEN** the Provider inventory request fails due to timeout, transport error, unavailable Provider API, or unreadable provider response
- **THEN** the Native Layer SHALL reject the request with `provider_api_unavailable`
- **AND** the Native Layer MUST NOT mutate the Workspace Catalog
