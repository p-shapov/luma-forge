## Why

GPU Cloud Provider Setup currently checks whether a RunPod key exists, awaits provider validation, and writes the key later. Concurrent setup requests can both observe missing setup and both report success, violating the native-owned setup lifecycle contract.

Workspace creation also depends on provider setup remaining complete while a Draft Workspace is validated and persisted. Provider setup deletion can currently interleave with that prerequisite window, allowing Workspace creation to succeed against stale setup state.

## What Changes

- Add a native-owned coordination boundary for provider setup state transitions, keyed by GPU Cloud Provider.
- Serialize create and delete operations for the same provider so each operation evaluates against the latest durable keyring state.
- Ensure concurrent setup requests for the same provider produce at most one successful setup.
- Ensure setup success is reported only after the stored key is re-read and live setup status is derived from durable state.
- Ensure Workspace creation and provider setup deletion cannot interleave in a way that persists a Workspace after provider setup becomes incomplete.
- Preserve existing command shapes, generated frontend bindings, and public error codes.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gpu-cloud-provider-setup`: Add concurrency guarantees for setup/create/delete state transitions and align setup success with re-read durable status.
- `workspace-setup`: Add a concurrency guarantee that Workspace creation validates provider setup consistently through persistence when provider setup deletion is requested concurrently.

## Impact

- Affected native code: provider setup service, provider setup command wiring, Workspace Setup service or command wiring, and any shared native state used to coordinate provider-keyed operations.
- Affected tests: provider setup concurrency tests, setup post-write re-read behavior tests, provider setup delete interleaving tests, and Workspace creation versus provider setup deletion race tests.
- No frontend API changes are expected.
- No storage schema migration is expected.
