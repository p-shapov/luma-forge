## Serverless Endpoint

A remote callable inference endpoint managed by the GPU Cloud Provider.

The Serverless Endpoint uses the GPU selected in the Placement Plan and runs the Endpoint Worker required for the Workspace on prepared Persistent Storage Volume.

**Invariants:**

- Each Workspace owns its own Serverless Endpoint.
- Serverless Endpoints are not shared between Workspaces.
- The Serverless Endpoint is the persistent runtime entry point after successful provisioning.

## See Also

- [GPU Cloud Provider](./gpu-cloud-provider.md)
- [Provider Resource](./provider-resource.md)
- [Persistent Storage Volume](./persistent-storage-volume.md)
- [Placement Plan](./placement-plan.md)
- [Endpoint Worker](./endpoint-worker.md)
- [Workspace](./workspace.md)