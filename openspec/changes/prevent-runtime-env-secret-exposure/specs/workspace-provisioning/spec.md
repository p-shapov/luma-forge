## ADDED Requirements

### Requirement: Exclude RunPod Template Runtime Environment From Workspace Metadata
Workspace Provisioning SHALL treat RunPod serverless template runtime environment values as transient provider observation data and MUST NOT persist or return those values as Workspace metadata.

#### Scenario: Template is created from provider observation
- **WHEN** Workspace Provisioning creates a RunPod serverless template and receives a provider observation that includes runtime environment values
- **THEN** the Native Layer SHALL persist the RunPod endpoint template snapshot without runtime environment keys or values
- **AND** the persisted snapshot SHALL retain the template id, endpoint worker image reference, mount path, and provider resource status needed for later provisioning and cleanup

#### Scenario: Existing template is adopted from provider discovery
- **WHEN** Workspace Provisioning adopts a safe Workspace-correlated RunPod serverless template discovered from the provider
- **AND** the discovered template observation includes runtime environment values
- **THEN** the Native Layer SHALL persist the RunPod endpoint template snapshot without runtime environment keys or values
- **AND** the Native Layer MUST NOT write Provider API Keys, worker bearer tokens, provider-owned env values, or operator-added template env values into Workspace Catalog metadata

#### Scenario: Existing template snapshot is refreshed
- **WHEN** Workspace Provisioning observes or validates a persisted RunPod endpoint template before creating or validating a Serverless Endpoint
- **AND** the provider observation includes runtime environment values
- **THEN** the Native Layer SHALL update only UI-safe template snapshot metadata
- **AND** the Native Layer MUST NOT add runtime environment keys or values to the Workspace metadata

#### Scenario: Legacy template metadata contains runtime environment
- **WHEN** Workspace Provisioning reads a Workspace whose existing RunPod endpoint template metadata contains a legacy runtime environment map
- **THEN** the Native Layer SHALL tolerate the legacy field for compatibility
- **AND** any subsequent persisted Workspace snapshot SHALL omit the legacy runtime environment map
- **AND** provisioning continuation SHALL use only safe template metadata for reuse, endpoint creation, readiness validation, and cleanup
