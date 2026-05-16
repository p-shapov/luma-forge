## Why

Provisioner dependency installation can remain at the same worker status for several minutes while `pip` is still running, but the worker currently discards subprocess output. This makes it impossible to tell from provider pod logs whether installation is downloading, building wheels, retrying, or blocked.

## What Changes

- Stream provisioner subprocess output, including `pip install`, to the worker process console so provider pod logs show active installation details.
- Keep `/status` responses UI-safe and avoid exposing raw command output, request payloads, credentials, stack traces, or environment dumps through the API.
- Preserve existing subprocess failure handling, cancellation, and timeout behavior while improving diagnostic visibility.
- Add tests proving subprocess output is emitted to console logs and remains excluded from worker status payloads.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `provisioner-worker`: Require long-running subprocess work to emit provider-visible console logs while preserving sanitized status responses.

## Impact

- Affected worker code: `workers/provisioner/src/auxiliary/command_runner.py`, dependency installation paths in `workers/provisioner/src/runtime/python_environment.py`, and related tests.
- No breaking changes to the worker HTTP API.
- No new third-party dependencies are expected.
- Provider-specific orchestration remains unchanged; the behavior relies on standard container stdout/stderr log capture.
