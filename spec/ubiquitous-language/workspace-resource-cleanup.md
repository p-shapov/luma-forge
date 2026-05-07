## Workspace Resource Cleanup

A user-triggered operation that removes one Workspace and all provisioning-related data associated with it.

**Invariants:**

- Workspace Resource Cleanup must be initiated manually by the user.
- Workspace Resource Cleanup does not delete Provider Resources belonging to other Workspaces.
- It does not delete unrelated resources in the user's GPU Cloud Provider account.
- Workspace Resource Cleanup must delete all Provider Resources referenced by the selected Workspace metadata.
- Workspace Resource Cleanup must remove the selected Workspace entry from the Workspace Catalog.
- Workspace Resource Cleanup must tolerate already-missing Provider Resources.

## See Also

- [Workspace](./workspace.md)
- [Workspace Catalog](./workspace-catalog.md)
- [Provider Resource](./provider-resource.md)
- [GPU Cloud Provider](./gpu-cloud-provider.md)
