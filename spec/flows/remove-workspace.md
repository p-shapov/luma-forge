# Remove Workspace

Remove one saved Workspace from the local Workspace Catalog after cleaning up provider resources recorded in the Workspace metadata.

## Scope

- Runs only after a Workspace exists in the Workspace Catalog.
- Applies to `Draft`, `Ready`, and `Failed` Workspaces.
- Does not delete Workspaces that are actively `Provisioning`.
- Does not delete Provider Resources that are not referenced by the selected Workspace metadata.

## Actors And Ownership

- User initiates removal manually from the local application.
- React requests removal but treats Native Layer responses as authoritative.
- Native Layer owns lifecycle validation, provider cleanup, Workspace Catalog deletion, and command errors.
- GPU Cloud Provider owns remote resources that may need deletion.

## Flow

1. User selects one Workspace to remove.
2. React calls `delete_workspace` with the selected Workspace id.
3. Native Layer loads the Workspace from the Workspace Catalog.
4. Native Layer rejects the request if the Workspace does not exist or is `Provisioning`.
5. Native Layer deletes provider resources referenced by the Workspace metadata when cleanup metadata exists.
6. Native Layer deletes the Workspace Catalog entry only after cleanup succeeds.
7. Native Layer returns the updated Workspace Catalog.

## Failure Handling

- Missing Workspace is rejected with `workspace_not_found`.
- `Provisioning` Workspace removal is rejected with `invalid_workspace_lifecycle`.
- Cleanup failure returns a UI-safe provider, keyring, token, or cleanup error.
- If cleanup fails, Native Layer preserves the Workspace Catalog entry and cleanup metadata so the user can retry.
- Provider resources that are already missing are treated as successfully cleaned up.

## Security

- Removal must not expose Provider API Keys, worker bearer tokens, Hugging Face keys, provider transport payloads, or raw provider errors to React.
- Command responses include only UI-safe Workspace Catalog data.

## See Also

- [Workspace](../ubiquitous-language/workspace.md)
- [Workspace Catalog](../ubiquitous-language/workspace-catalog.md)
- [Workspace Resource Cleanup](../ubiquitous-language/workspace-resource-cleanup.md)
