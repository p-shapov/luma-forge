## Health Check

A validation process that checks whether the Workspace resources still exist and are usable.

Health Check may check:

- whether the Provider API Key is valid
- whether required active Provider Resources exist and are valid

Health Check may run:

- when the application starts
- when the application window receives focus

**Invariants:**

- Health Check validates the particular Workspace.
- Health Check validates that Provider API Key still exist and usable.
- Health Check validates that required active Provider Resources still exist and are usable.
- Health Check must not perform automatic reconcile or repair.

## See Also

- [Workspace](./workspace.md)
- [Provider API Key](./provider-api-key.md)
- [Provider Resource](./provider-resource.md)
