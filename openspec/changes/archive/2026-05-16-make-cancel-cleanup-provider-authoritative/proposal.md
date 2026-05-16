## Why

Workspace Provisioning cancellation currently treats Provisioner Worker `/cancel` failure as a cancellation cleanup failure even when Native can still delete all Workspace-owned RunPod resources. This can leave a Workspace in `failed` after the authoritative cleanup path has succeeded, forcing unnecessary manual recovery.

## What Changes

- Make provider resource deletion the authoritative success criterion for Workspace Provisioning cancellation cleanup.
- Remove the Provisioner Worker `/cancel` call from destructive Workspace Provisioning cancellation cleanup.
- Remove `POST /cancel` from the Provisioner Worker HTTP API because destructive cancellation now terminates the provider pod instead of gracefully cancelling an in-pod job.
- Stop relying on the temporary in-pod worker API during cancellation; terminating the RunPod provisioning pod is the authoritative stop signal.
- Continue to fail cancellation when Native cannot confirm deletion of known Provider Resources or cannot complete required local cleanup.
- Preserve existing secret-safety and cleanup metadata guarantees on failed cleanup.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `workspace-provisioning`: cancellation cleanup requirements change so destructive cancellation does not call worker `/cancel` and succeeds based on provider resource cleanup.
- `provisioner-worker`: the worker API no longer exposes or supports `POST /cancel`.

## Impact

- Affected native code: Workspace Provisioning cancellation service and shared known-resource cleanup behavior.
- Affected worker code: Provisioner Worker HTTP routing, request schemas, job manager cancellation entry point, and worker README.
- Affected tests: cancellation cleanup tests should assert that worker `/cancel` is not invoked during destructive cancellation; worker API tests should assert `POST /cancel` is not available.
- Affected specs: `workspace-provisioning` cancellation and shared cleanup requirements; `provisioner-worker` HTTP API requirements.
- No frontend command contract, generated binding, provider API, or dependency changes are expected.
