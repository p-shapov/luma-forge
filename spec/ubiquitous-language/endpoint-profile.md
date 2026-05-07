## Endpoint Profile

A manifest stored in the application sources.

The Endpoint Profile contains metadata required to start the Endpoint Worker.

An Endpoint Profile includes:

- Docker image reference
- Docker image digest
- GPU Cloud Provider specific metadata and configuration

**Invariants:**

- The Endpoint Profile is specific for selected GPU Cloud Provider.
- The Endpoint Profile may vary for different Workflow Preset execution types.
- The Endpoint Profile is not user data.

## See Also

- [GPU Cloud Provider](./gpu-cloud-provider.md)
- [Workflow Preset](./workflow-preset.md)
- [Endpoint Worker](./endpoint-worker.md)
