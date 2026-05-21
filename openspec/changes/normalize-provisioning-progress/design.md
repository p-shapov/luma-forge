## Context

Workspace Provisioning exposes `WorkspaceProvisioningProgress` to React as a UI-safe rendering and sync-loop hint. The current implementation derives coarse phases from persisted Workspace metadata, but `progress_from_worker_status` passes `ProvisionerWorkerStatus.progress_percent` through directly when the worker is preparing the environment.

That pass-through makes a worker-local percentage look like total provisioning progress. It also reports worker startup and readiness lag as `preparing_environment`, even though environment preparation has not begun until the worker job is accepted and running.

## Goals / Non-Goals

**Goals:**

- Make `WorkspaceProvisioningProgress.percent` represent total Workspace Provisioning progress whenever it is present.
- Preserve Native ownership of progress semantics and keep React limited to rendering the returned value.
- Map worker-local preparation progress into a stable global `preparing_environment` range.
- Treat Provisioner Worker HTTP readiness lag and job start as part of `starting_provisioning_pod`.
- Keep endpoint template work under `creating_endpoint`, matching the existing Workspace Provisioning contract.

**Non-Goals:**

- Do not add a separate public `creating_endpoint_template` phase.
- Do not add persisted progress history or durable last-known percentage storage.
- Do not define a separate cancellation cleanup progress model.
- Do not change provider resource sequencing, idempotency, cleanup semantics, or worker API payload shape.

## Decisions

### Native maps every active phase to total progress anchors

Use fixed global progress anchors in Native:

| Phase | Percent |
| --- | ---: |
| `not_started` | `0` |
| `creating_volume` | `0` |
| `starting_provisioning_pod` | `10` |
| `preparing_environment` | `40..90` |
| `creating_endpoint` | `90` |
| `validating_readiness` | `98` |
| `completed` | `100` |
| `cleaning_up` | `null` |
| `failed` | `null` |

Rationale: the Native Layer owns durable provisioning decisions and already exposes progress as part of the generated command contract. Keeping the math in the Workspace Provisioning state machine prevents React or the Provisioner Worker adapter from encoding workflow semantics.

Alternative considered: let React convert phase and worker percent into UI progress. This would make React depend on Native-owned workflow decisions and would duplicate progress semantics outside the authoritative layer.

Alternative considered: let `workspace_provisioner` convert worker status into `WorkspaceProvisioningProgress`. That keeps React thin, but it still puts Workspace Provisioning phase ownership in the worker adapter. The worker adapter should report worker facts and persistence outcomes; Workspace Provisioning should decide the user-facing phase and total progress.

### Worker progress is local to environment preparation

When the Provisioner Worker reports running preparation progress, the Workspace Provisioning state machine maps it into the global `40..90` range:

```text
overall_percent = 40 + floor(worker_percent * 50 / 100)
```

If the worker does not provide a percent for a running preparation status, Native returns the `preparing_environment` lower bound of `40`.

Rationale: the worker owns only the preparation subtask. A bounded mapping lets the UI reflect real nested progress without implying that the worker can measure volume, pod, endpoint, or readiness work.

Alternative considered: have the worker report global progress directly. This would couple the container-side worker to provider orchestration phases that it does not own.

### Worker startup remains part of starting the provisioning pod

Provisioner Worker startup includes waiting for the pod to become usable, reaching the worker HTTP API, observing idle status, and accepting the start request. Native reports this as `starting_provisioning_pod` at `10`.

`preparing_environment` begins only after Workspace Provisioning receives a worker outcome showing an accepted/running preparation job or concrete worker preparation progress.

Rationale: until the worker job has started, the user-facing work is still getting temporary provisioning compute and its worker process online. This gives `preparing_environment` a clearer meaning: the worker is doing useful workspace preparation.

Alternative considered: keep readiness lag under `preparing_environment`. That preserves current behavior but makes the phase name cover both startup waiting and actual preparation.

### Cancellation and failure do not reuse provisioning-to-ready progress

Cancellation cleanup and failed states return `percent: null` unless a separate cleanup progress model is designed later.

Rationale: cleanup is not progress toward a Ready workspace, and failed state currently derives from durable failure metadata without persisted progress history.

Alternative considered: preserve the last observed percent on failure. That requires durable or response-level last-progress tracking and is outside the scope of this normalization.

## Risks / Trade-offs

- Existing UI expectations may change because progress will appear lower during environment preparation than the old worker-local pass-through. -> Update tests and rely on the clearer total-progress contract.
- Fixed anchors are estimates, not provider-measured progress. -> Treat anchors as lower-bound checkpoints and only use worker-local percent for the bounded preparation range.
- Worker start responses may have phase aliases that currently map directly to `preparing_environment`. -> Centralize mapping in Workspace Provisioning and add tests for idle, readiness lag, accepted start, running worker progress, succeeded worker, cleanup, and failed cases.
- Floor-based scaling can make progress appear to pause for small worker increments. -> Keep integer percentages stable and monotonic; smooth animation, if desired, remains a UI concern later.
