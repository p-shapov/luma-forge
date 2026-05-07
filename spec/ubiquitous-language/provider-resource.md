## Provider Resource

A remote infrastructure object created and managed on the GPU Cloud Provider side for a Workspace.

Provider Resource is an application-level abstraction. Each GPU Cloud Provider may represent these resources differently.

Provider Resources may include:

- Persistent Storage Volume
- Provisioning Pod
- Serverless Endpoint

**Invariants:**

- Active Provider Resources are referenced from local Workspace metadata.
- Workspace metadata must retain enough Provider Resource metadata for readiness checks, retry decisions, diagnostics, and Workspace Resource Cleanup.
- Each Workspace owns its own Provider Resource set.
- Provider Resources are not shared between Workspaces.

## See Also

- [GPU Cloud Provider](./gpu-cloud-provider.md)
- [Workspace](./workspace.md)
- [Persistent Storage Volume](./persistent-storage-volume.md)
- [Provisioning Pod](./provisioning-pod.md)
- [Serverless Endpoint](./serverless-endpoint.md)
