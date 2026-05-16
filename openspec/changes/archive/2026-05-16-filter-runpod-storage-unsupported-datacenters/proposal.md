## Why

RunPod placement inventory can report GPU availability for datacenters that cannot create the network volumes required by LumaForge provisioning. This allows users to create workspaces that repeatedly fail during the first provisioning step with a retryable provider availability error even though the selected placement is structurally incompatible with the current provisioning model.

## What Changes

- Fetch RunPod datacenter storage capability as part of provider placement inventory.
- Exclude RunPod datacenters that cannot support the required persistent network volume from returned placement options.
- Show returned GPU availability in the Workspace Setup UI so zero-availability GPUs are visible before provisioning.
- Disable workspace creation and provisioning start controls when the selected GPU is currently known unavailable.
- Preserve the existing frontend-facing provider inventory shape unless a stronger domain capability field is required during implementation.
- Add tests that prevent storage-unsupported datacenters with available GPUs from being offered as valid RunPod placement options.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `workspace-setup`: Provider placement options must only expose RunPod datacenters that can support the persistent storage volume required by Workspace Provisioning, and the UI must surface returned GPU availability while blocking unavailable GPU selections from starting provisioning.

## Impact

- Affects RunPod provider inventory GraphQL query and mapping.
- Affects native Workspace Setup placement options returned to React.
- Affects frontend Workspace Setup display for GPU placement options.
- Requires native tests for RunPod inventory mapping and Workspace Setup placement option behavior.
