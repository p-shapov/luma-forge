## Provisioning Profile

A manifest stored in the application sources.

The Provisioning Profile contains metadata required to start and communicate with a Provisioning Pod.

A Provisioning Profile includes:

- profile id
- provisioner version
- Docker image reference
- Docker image digest
- GPU Cloud Provider metadata and configuration

The configuration may include:

- volume mount path
- container disk size
- compute type
- open port for provisioning progress reporting

**Invariants:**

- The Provisioning Profile is shared across Workspaces.
- The Provisioning Profile may vary depending on the selected GPU Cloud Provider.
- The Provisioning Profile is not user data.

## See Also

- [GPU Cloud Provider](./gpu-cloud-provider.md)
- [Provisioning Pod](./provisioning-pod.md)
- [Workspace](./workspace.md)
