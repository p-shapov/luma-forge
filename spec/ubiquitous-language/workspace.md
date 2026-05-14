## Workspace

A local user workspace entry in the Workspace Catalog that stores the configuration and metadata needed to work with a ready remote environment.

A Workspace includes:

- referenced GPU Cloud Provider identifier
- selected Placement Plan
- Provider Resource snapshot for the created Persistent Storage Volume
- timestamp proving the remote environment was prepared successfully
- Provider Resource snapshot for the active Provisioning Pod, while provisioning is active
- Provider Resource snapshot for the created Serverless Endpoint
- historical Provisioning Pod snapshot when needed for diagnostics or cleanup

Lifecycle states: `Draft`, `Provisioning`, `Ready`, `Failed`.

**Invariants:**

- There can be multiple Workspaces in one Workspace Catalog.
- A Workspace identifier must be unique inside the Workspace Catalog.
- No Provider Resources are shared between Workspaces.
- A Workspace must not be considered `Ready` until all required active Provider Resources are available and usable.
- `Draft` means Workspace metadata exists and no provider mutation has started.
- `Provisioning` means Native Layer is creating Provider Resources, running the Provisioner Worker, terminating temporary provisioning compute, creating the Serverless Endpoint, or validating readiness.
- `Provisioning` Workspaces are resumable by provisioning sync from persisted Workspace metadata and Provider Resource snapshots.
- `environment_prepared_at` is set only after the Provisioner Worker succeeds and the Provisioning Pod is terminated or confirmed absent.
- `Ready` means the persistent runtime entry point exists and required Provider Resources passed readiness validation.
- If provisioning was failed, the Workspace must be marked `Failed` and must retain Provider Resource snapshot required for Workspace Resource Cleanup.
- `Failed` Workspace recovery is cleanup-first in v1: the user runs Workspace Resource Cleanup or Factory Reset and starts Workspace Creation again.

## See Also

- [Workspace Catalog](./workspace-catalog.md)
- [Workspace Provisioning Progress](./workspace-provisioning-progress.md)
- [GPU Cloud Provider](./gpu-cloud-provider.md)
- [Workflow Preset](./workflow-preset.md)
- [Placement Plan](./placement-plan.md)
- [Serverless Endpoint](./serverless-endpoint.md)
- [Persistent Storage Volume](./persistent-storage-volume.md)
- [Provisioning Pod](./provisioning-pod.md)
- [Provider Resource](./provider-resource.md)
- [Workspace Resource Cleanup](./workspace-resource-cleanup.md)
- [Factory Reset](./factory-reset.md)
