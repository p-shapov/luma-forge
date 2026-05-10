## 1. Repository Read Consistency

- [x] 1.1 Update SQLite Workspace Catalog reads to select indexed row columns alongside `workspace_json`.
- [x] 1.2 Add a shared row decoding path that deserializes `workspace_json` and validates indexed `id`, `name`, `gpu_cloud_provider_id`, `lifecycle_state`, and `workflow_preset_id` against the decoded Workspace.
- [x] 1.3 Return `WorkspaceSetupError::WorkspaceCatalogUnavailable` when row decoding or row/payload consistency validation fails.
- [x] 1.4 Ensure both `list_workspaces` and the post-insert re-read path use the shared validation logic.

## 2. Tests

- [x] 2.1 Add a test proving `list_workspaces` rejects a row whose indexed provider id disagrees with `workspace_json`.
- [x] 2.2 Add tests for the other duplicated indexed fields: `id`, `name`, `lifecycle_state`, and `workflow_preset_id`.
- [x] 2.3 Add or adjust a test proving a valid insert still re-reads and returns the serialized Workspace payload after consistency validation.

## 3. Verification

- [x] 3.1 Run `cargo test`.
- [x] 3.2 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 3.3 Run `cargo fmt`.
