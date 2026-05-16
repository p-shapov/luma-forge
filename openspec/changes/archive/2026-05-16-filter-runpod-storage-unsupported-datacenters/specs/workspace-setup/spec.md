## MODIFIED Requirements

### Requirement: Read provider placement inventory

The Native Layer SHALL expose a `get_provider_placement_options` command that returns provider placement options for an explicit GPU Cloud Provider after validating local provider setup prerequisites. Placement options SHALL include live Provider Inventory and provider placement capabilities, and provider-specific inventory mapping SHALL only expose datacenters that can satisfy LumaForge's current provisioning prerequisites.

#### Scenario: Provider setup is complete

- **WHEN** the Client requests provider placement options for `runpod` and the local Provider API Key exists
- **THEN** the Native Layer SHALL call RunPod using the stored Provider API Key to fetch live Provider Inventory
- **AND** the Native Layer SHALL return available datacenters, GPU options per datacenter, and provider maximum Persistent Storage Volume size when known
- **AND** returned RunPod datacenters SHALL be limited to datacenters whose provider inventory reports support for persistent network storage
- **AND** the Native Layer SHALL return placement capabilities for RunPod endpoint keep-alive with `supported = true`, `default_seconds = 5`, `min_seconds = 5`, and `max_seconds = 3600`
- **AND** the response MUST NOT include the Provider API Key

#### Scenario: RunPod datacenter has GPUs but does not support network storage

- **WHEN** RunPod inventory reports a datacenter with GPU availability and storage support is false or missing
- **THEN** the Native Layer SHALL omit that datacenter from returned provider placement options
- **AND** the Native Layer MUST NOT expose the omitted datacenter as selectable for a new RunPod Placement Plan

#### Scenario: GPU availability is displayed during placement selection

- **WHEN** the Native Layer returns RunPod GPU options with availability scores
- **THEN** the Client SHALL display the availability for each selectable GPU option
- **AND** the Client SHALL surface zero availability as unavailable rather than hiding the GPU solely because the score is zero
- **AND** the Client SHALL disable workspace creation while the selected GPU has zero availability
- **AND** the Client SHALL disable starting provisioning for a selected workspace when loaded placement options show that workspace's selected GPU has zero availability or is absent from currently eligible placement options
- **AND** the Client SHALL continue allowing provisioning sync and cancellation commands for existing workspaces regardless of current GPU availability

#### Scenario: Provider setup is incomplete

- **WHEN** the Client requests provider placement options and the required local Provider API Key is missing
- **THEN** the Native Layer SHALL reject the request with `provider_setup_incomplete`
- **AND** the Native Layer MUST reject before calling the Provider

#### Scenario: Provider API Key is invalid or revoked

- **WHEN** the Client requests provider placement options and the Provider rejects the stored Provider API Key as unauthorized or forbidden
- **THEN** the Native Layer SHALL reject the request with `invalid_provider_api_key`
- **AND** the Native Layer MUST NOT report the failure as retryable
- **AND** the Native Layer MUST NOT mutate the Workspace Catalog

#### Scenario: Provider inventory request is rate limited

- **WHEN** the Provider inventory request fails because the Provider reports rate limiting
- **THEN** the Native Layer SHALL reject the request with a retryable UI-safe `provider_rate_limited` command error
- **AND** the Native Layer MUST NOT mutate the Workspace Catalog
- **AND** the command error MUST NOT expose Provider API Keys, raw provider request bodies, raw provider response bodies, or provider-specific error codes

#### Scenario: Provider inventory request is temporarily unavailable

- **WHEN** the Provider inventory request fails due to timeout, transport error, or temporarily unavailable Provider API
- **THEN** the Native Layer SHALL reject the request with a retryable UI-safe provider availability error
- **AND** the Native Layer MUST NOT mutate the Workspace Catalog
- **AND** the command error MUST NOT expose Provider API Keys, raw provider request bodies, raw provider response bodies, or provider-specific error codes

#### Scenario: Provider inventory request is rejected

- **WHEN** the Provider rejects the inventory request for a non-authentication request validation reason
- **THEN** the Native Layer SHALL reject the request with a non-retryable UI-safe `provider_request_rejected` command error
- **AND** retryability SHALL be derived from the LumaForge-owned provider error instead of provider-specific error codes or message strings
- **AND** the Native Layer MUST NOT mutate the Workspace Catalog

#### Scenario: Provider inventory response is invalid

- **WHEN** the Provider inventory request succeeds but the Provider response cannot be parsed or cannot be mapped into valid Provider Inventory
- **THEN** the Native Layer SHALL reject the request with a UI-safe provider inventory or response validation error
- **AND** the Native Layer MUST NOT report the same request as safely retryable solely because parsing failed
- **AND** the Native Layer MUST NOT mutate the Workspace Catalog

#### Scenario: Provider does not support endpoint keep-alive

- **WHEN** a future GPU Cloud Provider does not support endpoint keep-alive configuration
- **THEN** its provider placement options SHALL return endpoint keep-alive capability with `supported = false`
- **AND** that provider's Placement Plan variant MUST NOT persist an endpoint keep-alive value
