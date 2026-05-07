## Workspace Catalog

A local user-owned catalog of Workspaces known to the desktop application.

The Workspace Catalog is the authoritative local collection used to list, find, create, update, and remove Workspace metadata records.

**Invariants:**

- The Workspace Catalog can contain zero, one, or many Workspaces.
- Each Workspace identifier must be unique inside the Workspace Catalog.
- Native Layer (Rust / Tauri) owns all durable Workspace Catalog mutations.
- Client (React) treats Workspace Catalog data returned by Native Layer as authoritative.
- Workspace Setup adds a complete `Draft` Workspace to the Workspace Catalog.
- Workspace Provisioning updates one existing Workspace entry in the Workspace Catalog.
- Workspace Resource Cleanup removes one Workspace entry from the Workspace Catalog after handling the selected Workspace resources according to cleanup rules.
- Factory Reset removes all Workspace entries from the Workspace Catalog after handling referenced resources according to reset rules.
- The Workspace Catalog does not contain Provider API Keys.
- The Workspace Catalog does not create or own Provider Resources; it only stores Workspace metadata that references them.

## See Also

- [Workspace](./workspace.md)
- [Workspace Resource Cleanup](./workspace-resource-cleanup.md)
- [Factory Reset](./factory-reset.md)
