## Context

The Provisioner Worker has three error surfaces: process startup diagnostics, immediate HTTP API errors, and terminal job failure metadata returned through `GET /status`. Startup configuration validation now emits a structured diagnostic with `code`, `reason_code`, `message`, and context fields, while runtime worker errors still mostly use only `code` and `message`.

The future native provisioning consumer needs predictable classification without parsing free-form messages. The worker should keep existing top-level `code` values for broad compatibility, but add stable reason-level classification and structured context across all runtime error surfaces.

## Goals / Non-Goals

**Goals:**

- Use one common worker error payload shape for immediate HTTP errors and job failure metadata.
- Preserve existing broad `code` values such as `invalid_request`, `unauthorized`, `request_too_large`, `job_already_running`, and preparation failure codes.
- Add `reason_code` for specific handling without making consumers parse `message`.
- Keep optional context structured and secret-free.
- Document the contract and cover it with worker tests.

**Non-Goals:**

- Do not change the startup configuration error shape introduced by `harden-provisioner-runtime-env`.
- Do not change HTTP status codes.
- Do not expose stack traces, raw command output, request bodies, bearer tokens, provider API keys, or credential-bearing URLs.
- Do not introduce a native provisioning orchestrator or generated TypeScript binding changes in this proposal.

## Decisions

### Introduce a shared worker error payload builder

`WorkerError` should own a `to_dict()` or equivalent serialization method that returns the canonical payload. API handlers and job status construction should use that method instead of hand-building error dictionaries.

Alternative considered: keep separate payload construction in `api.py` and `job_manager.py`. That would make response drift likely as new error classes are added.

### Keep `code` broad and add `reason_code` for specificity

Existing `code` values remain the primary classification and should not be renamed. Each error class gains a stable `reason_code` that identifies the exact cause or default reason for that class. For example, `invalid_request` can carry `invalid_json`, `malformed_content_length`, or `invalid_start_payload`; `job_already_running` can carry `active_job_exists`.

Alternative considered: replace `code` with a more granular enum. That would be a larger breaking change for consumers and tests.

### Use structured context only when needed

Payloads should support an optional `context` object for safe metadata such as `active_job_id`, `field`, `max_request_bytes`, or `phase`. Values must be explicitly allowlisted by the error class or creation site. Conflict responses should move `active_job_id` into `context.active_job_id` while retaining compatibility only if the implementation intentionally chooses a temporary legacy field.

Alternative considered: allow arbitrary details from exceptions. That risks leaking secrets and raw command output.

### Align terminal job failures with immediate API errors

When a provisioning step fails, `GET /status.error` should contain the same canonical shape as an immediate API error. The top-level status fields remain unchanged.

## Risks / Trade-offs

- Consumers may already read `active_job_id` at the top level of conflict responses -> document whether implementation keeps a compatibility alias or moves it exclusively into `context`.
- More granular reason codes require maintenance -> define defaults on each error class and add tests for representative payloads.
- Validation errors may need field-level context -> only include field names, never raw invalid values.
- This change overlaps conceptually with startup configuration diagnostics -> keep startup process errors separate but aligned in shape.
