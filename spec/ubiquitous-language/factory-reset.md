## Factory Reset

A user-triggered operation that removes all Workspaces, all provisioning-related data associated with them.

Factory Reset removes:

- all Workspace entries in the Workspace Catalog
- all Provider Resources referenced by Workspace Catalog entries
- Provider API Key stored in the local secure keyring

**Invariants:**

- Factory Reset must be initiated manually by the user.
- Factory Reset must tolerate already-missing Provider Resources.
- Factory Reset must not delete unrelated resources in the user's GPU Cloud Provider account.

## See Also

- [Workspace](./workspace.md)
- [Workspace Catalog](./workspace-catalog.md)
- [Provider API Key](./provider-api-key.md)
- [Provider Resource](./provider-resource.md)
- [Workspace Resource Cleanup](./workspace-resource-cleanup.md)
