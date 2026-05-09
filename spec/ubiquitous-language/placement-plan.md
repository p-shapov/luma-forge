## Placement Plan

A provider-specific, user-selected configuration that describes how remote infrastructure should be created before provisioning starts.

The Placement Plan is configured for selected GPU Cloud Provider and Workflow Preset specifically.

Placement Plan may include:

- selected data center or geo for GPU Cloud Provider
- selected GPU reported as available in that data center when placement options were fetched
- selected Workflow Preset
- selected Endpoint Profile
- selected Provisioning Profile
- requested Persistent Storage Volume size

The Placement Plan is used by the provisioning flow to create Provider Resources such as:

- Persistent Storage Volume
- Provisioning Pod
- Serverless Endpoint

**Invariants:**

- The Placement Plan may vary depending on the selected GPU Cloud Provider.
- The minimum required Persistent Storage Volume size is explicitly declared by the selected Workflow Preset.
- The requested Persistent Storage Volume size is the final size used for provisioning.
- The Client may calculate the requested Persistent Storage Volume size from the preset-declared minimum plus user-selected additional size before submitting Workspace Setup.
- Workspace Setup stores selected data center and GPU identifiers as requested placement values. Provider-owned flows validate whether those values are still available before creating or mutating Provider Resources.
- The Placement Plan is not a Provider Resource.

## See Also

- [GPU Cloud Provider](./gpu-cloud-provider.md)
- [Workflow Preset](./workflow-preset.md)
- [Provider Resource](./provider-resource.md)
- [Persistent Storage Volume](./persistent-storage-volume.md)
- [Provisioning Pod](./provisioning-pod.md)
- [Provisioning Profile](./provisioning-profile.md)
- [Endpoint Profile](./endpoint-profile.md)
- [Serverless Endpoint](./serverless-endpoint.md)
