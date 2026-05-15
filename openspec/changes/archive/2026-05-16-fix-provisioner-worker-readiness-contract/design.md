## Context

Workspace Provisioning currently observes a RunPod provisioning pod, then contacts the Provisioner Worker as soon as the provider reports the pod as `running`. RunPod can report the pod running before the HTTP proxy and worker process are ready, so Native may emit `provisioner_worker_unavailable` during normal startup lag. Once the worker becomes reachable, Native can still reject the current worker contract: the worker reports idle as `status: "idle"` with `phase: null`, while Native requires a non-null phase string before it will call `/start`.

The current worker API also uses `job_id` for start/cancel correlation and exposes `diagnostic_message` plus structured `error` metadata, while Native still sends `workspace_id` and only parses a `diagnostic` field. This prevents useful diagnosis of real preparation failures.

## Goals / Non-Goals

**Goals:**

- Treat worker transport failures during pod startup as expected readiness lag while retaining retryable command errors for genuine unavailable cases outside safe provisioning progress.
- Make Native consume the current Provisioner Worker API without requiring React to understand worker-specific response details.
- Preserve Native as the authoritative owner of durable provisioning state and failure classification.
- Keep all worker diagnostics UI-safe and secret-safe.
- Add tests that reproduce the observed failure path before implementation and prove the contract alignment after implementation.

**Non-Goals:**

- Do not change RunPod as the only v1 GPU cloud provider.
- Do not add a hosted backend, local ML execution, or frontend-owned provisioning state.
- Do not redesign workspace cleanup or resource recovery beyond preserving known cleanup metadata.
- Do not expose bearer tokens, provider API keys, raw command output, stack traces, or request bodies through command responses or logs.

## Decisions

1. Native will model worker readiness as provisioning progress, not durable failure.

   When the provider pod is running but `GET /status` fails due to connection refusal, timeout, DNS/proxy startup lag, TLS/proxy failure, a retryable 5xx response, or a non-worker proxy response that is not JSON, the sync command will return authoritative Workspace metadata with running progress. It will not mark the Workspace failed and will not surface a user-facing `provisioner_worker_unavailable` error while the active pod remains a safe continuation path.

   Alternative considered: increase the HTTP timeout and keep returning command errors. This would reduce some transient errors but would still make normal startup lag look like a failure and would block progress rendering.

2. Native will adapt to the worker's published API contract.

   The Native worker gateway will send `job_id` in `/start` and `/cancel` requests. It will parse `status`, nullable `phase`, `progress_percent`, `diagnostic_message`, and structured `error`. For Native domain progress, nullable idle/terminal phases will map to appropriate Workspace Provisioning phases rather than invalid payloads.

   Alternative considered: change the worker to accept Native's current `workspace_id` and non-null phase vocabulary. The worker contract is already documented and covered by worker tests, and Native is the adapter to provider/worker-specific details, so adapting Native keeps the boundary cleaner.

3. Worker status vocabulary will be mapped at the boundary.

   Native will preserve its own Workspace Provisioning phases and map worker phases such as `starting`, `installing_comfyui`, `installing_custom_nodes`, `downloading_assets`, `validating_environment`, and any finalizing/writing-manifest phase into `preparing_environment` or the next native provisioning phase. Unknown successful payload fields that are not needed by Native may be ignored, but unknown statuses or unsafe progress percentages remain invalid.

   Alternative considered: introduce every worker phase into the domain model. That would leak worker implementation detail into Workspace Provisioning and generated frontend contracts.

4. Worker error classification will distinguish transport readiness from API contract failures.

   Authorization failures remain authorization failures. Conflicts remain conflicts. Worker validation errors and malformed JSON success payloads from the worker become response/request invalid classifications with sanitized diagnostics. Non-JSON proxy readiness responses remain temporary unavailability. Terminal worker job failures and worker API contract failures persist structured failure details from the worker's safe `error` payload and `diagnostic_message` when available. Retryable transport failures during safe pod startup do not become durable failures.

   Alternative considered: keep all non-success statuses as `ProvisionerWorkerUnavailable`. That was the source of ambiguous diagnostics and hid contract drift.

## Risks / Trade-offs

- Worker readiness could mask a genuinely broken image that never starts the API. Mitigation: keep sync retryable and observable, continue observing provider pod status, and let terminal provider pod failure or explicit worker terminal errors produce durable failure.
- Worker phase mapping may drift again. Mitigation: centralize mapping in the Native worker gateway and add tests for the documented worker phase vocabulary.
- Sanitized diagnostics may omit details needed for debugging. Mitigation: persist stable worker `code` and `reason_code` where available, plus bounded UI-safe messages, while keeping raw command output and secrets excluded.
- Existing failed local workspaces will remain failed. Mitigation: this change affects future provisioning attempts; cleanup metadata remains available for manual or future cleanup flows.
