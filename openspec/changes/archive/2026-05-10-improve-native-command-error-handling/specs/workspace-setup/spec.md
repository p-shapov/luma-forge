## ADDED Requirements

### Requirement: Workspace Setup read commands report source-specific catalog failures

Workspace Setup read commands SHALL report which local catalog or profile source failed instead of collapsing every bundled catalog/profile failure into Workflow Catalog unavailability.

#### Scenario: Workflow Catalog read fails

- **WHEN** the Native Layer cannot parse, validate, or return the bundled Workflow Catalog
- **THEN** `get_workflow_catalog` SHALL reject with `workflow_catalog_unavailable` or a more specific Workflow Catalog invalid/unavailable code

#### Scenario: Provisioning Profiles read fails

- **WHEN** the Native Layer cannot parse, validate, or return bundled Provisioning Profiles
- **THEN** `get_provisioning_profiles` SHALL reject with a Provisioning Profiles-specific UI-safe code
- **AND** the command MUST NOT return `workflow_catalog_unavailable` solely because Provisioning Profiles failed

#### Scenario: Endpoint Profiles read fails

- **WHEN** the Native Layer cannot parse, validate, or return bundled Endpoint Profiles
- **THEN** `get_endpoint_profiles` SHALL reject with an Endpoint Profiles-specific UI-safe code
- **AND** the command MUST NOT return `workflow_catalog_unavailable` solely because Endpoint Profiles failed

### Requirement: Provider Inventory reports provider failure classes precisely

Provider Inventory reads SHALL distinguish Provider authorization, provider network/API availability, malformed responses, and invalid mapped inventory.

#### Scenario: Stored Provider API Key is missing

- **WHEN** the Client requests Provider Inventory and the required local Provider API Key is missing
- **THEN** the Native Layer SHALL reject the request with `provider_setup_incomplete`

#### Scenario: Stored Provider API Key is unauthorized

- **WHEN** RunPod rejects the stored Provider API Key while fetching inventory
- **THEN** the Native Layer SHALL reject the request with a Provider API Key authorization error
- **AND** React SHALL be able to route the user toward Provider Setup recovery

#### Scenario: Provider Inventory request cannot reach provider

- **WHEN** RunPod inventory lookup fails due to timeout, DNS, connection failure, request timeout, provider outage, rate limiting, or non-auth provider availability failure
- **THEN** the Native Layer SHALL reject the request with a retryable provider availability error

#### Scenario: Provider Inventory response is malformed or invalid

- **WHEN** RunPod inventory lookup returns a response that cannot be parsed, mapped, or validated as a Provider Inventory
- **THEN** the Native Layer SHALL reject the request with a Provider response or inventory invalid error
- **AND** the generated command error MUST NOT include the raw Provider response body

### Requirement: Workspace creation reports request validation failures precisely

Workspace creation SHALL return field-specific UI-safe errors for invalid command request shape before evaluating provider setup, catalogs, placement, or persistence.

#### Scenario: Workspace UUID is invalid

- **WHEN** the Client submits a Workspace creation request whose `workspace_id` is missing or is not a valid UUID
- **THEN** the Native Layer SHALL reject the request with `invalid_workspace_id`
- **AND** the Native Layer MUST NOT read Provider setup, bundled catalogs, Provider Inventory, or Workspace Catalog persistence

#### Scenario: Workspace name is missing

- **WHEN** the Client submits a Workspace creation request whose `name` is empty or blank after trimming
- **THEN** the Native Layer SHALL reject the request with `workspace_name_required`
- **AND** the Native Layer MUST NOT persist a Workspace record

#### Scenario: Workspace metadata construction fails

- **WHEN** the Native Layer cannot construct a valid Draft Workspace from otherwise parsed request data
- **THEN** the Native Layer SHALL reject the request with `invalid_workspace_metadata`
- **AND** the Native Layer MUST NOT persist a Workspace record

### Requirement: Workspace creation reports placement validation failures precisely

Workspace creation SHALL return UI-safe placement validation categories for incomplete, stale, or incompatible Placement Plans.

#### Scenario: Placement provider does not match request provider

- **WHEN** the Placement Plan provider, selected Provisioning Profile provider, or selected Endpoint Profile provider does not match the requested GPU Cloud Provider
- **THEN** the Native Layer SHALL reject the request with a placement provider mismatch error
- **AND** the Native Layer MUST NOT persist a Workspace record

#### Scenario: Placement selection is incomplete

- **WHEN** the Placement Plan is missing a selected datacenter or selected GPU
- **THEN** the Native Layer SHALL reject the request with a field-specific placement selection error
- **AND** the Native Layer MUST NOT persist a Workspace record

#### Scenario: Placement references stale catalog data

- **WHEN** the selected Workflow Preset, Provisioning Profile, or Endpoint Profile is absent from current bundled catalog data or does not exactly match current bundled catalog data
- **THEN** the Native Layer SHALL reject the request with a stale catalog object error identifying the stale object category
- **AND** React SHALL be able to prompt the user to reload catalogs and reselect placement data

#### Scenario: Endpoint profile is incompatible with workflow

- **WHEN** the selected Endpoint Profile workflow execution type does not match the selected Workflow Preset workflow execution type
- **THEN** the Native Layer SHALL reject the request with an endpoint/profile compatibility error
- **AND** the Native Layer MUST NOT persist a Workspace record

#### Scenario: Requested storage is below preset minimum

- **WHEN** the requested persistent storage volume size is smaller than the selected Workflow Preset required base volume size
- **THEN** the Native Layer SHALL reject the request with a storage minimum error
- **AND** React SHALL be able to identify that storage selection must be increased

### Requirement: Workspace Catalog command errors distinguish safe recovery categories

Workspace Catalog read and write failures SHALL expose safe command-level categories that help React choose retry, recovery, or blocking behavior.

#### Scenario: Local storage path is unavailable

- **WHEN** the Native Layer cannot resolve or create the app data directory or connect to the SQLite catalog file
- **THEN** Workspace Catalog commands SHALL reject with a local storage or Workspace Catalog storage unavailable error

#### Scenario: Workspace Catalog migration fails

- **WHEN** Workspace Catalog initialization cannot apply or validate required persistence migrations
- **THEN** Workspace Catalog commands SHALL reject with a Workspace Catalog migration failure error
- **AND** the command response MUST NOT expose raw SQL, raw SQLx errors, or raw migration implementation details

#### Scenario: Workspace Catalog data is corrupt or inconsistent

- **WHEN** a persisted Workspace row cannot be decoded, fails domain validation, or disagrees with its indexed SQLite row data
- **THEN** Workspace Catalog commands SHALL reject with a Workspace Catalog corruption or schema mismatch error
- **AND** the command response MUST NOT expose raw `workspace_json`

#### Scenario: Workspace UUID already exists

- **WHEN** the Client submits a Workspace UUID that already exists in the Workspace Catalog
- **THEN** the Native Layer SHALL continue to reject the request with `workspace_already_exists`
- **AND** the Native Layer MUST NOT mutate the existing Workspace record

### Requirement: Workspace Setup command errors guide frontend recovery

Workspace Setup command errors SHALL give React enough UI-safe information to present targeted recovery actions.

#### Scenario: Workspace Setup command fails

- **WHEN** any Workspace Setup read or mutation command fails
- **THEN** React SHALL be able to distinguish whether the user should retry, refresh provider setup, reload catalogs, refresh Workspace Catalog, reselect placement data, change a request field, or recover local storage
- **AND** React MUST NOT infer recovery behavior by parsing command error messages
