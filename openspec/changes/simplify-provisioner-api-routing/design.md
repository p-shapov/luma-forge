## Context

The provisioner worker exposes a small authenticated HTTP API from inside the provisioner container. Its request handler currently uses Python's stdlib `BaseHTTPRequestHandler` directly and performs path dispatch inline in `do_GET` and `do_POST`.

The worker is intentionally dependency-light. Its only runtime dependency is `huggingface_hub`, and the API surface is limited to three endpoints. The readability problem is local to `workers/provisioner/src/provisioner_worker/api.py`; introducing a framework would create more dependency and runtime surface than the API currently warrants.

## Goals / Non-Goals

**Goals:**

- Make route dispatch in `api.py` easier to scan and maintain.
- Separate endpoint-specific actions from shared HTTP concerns such as authorization, JSON parsing, request-size checks, and response writing.
- Preserve existing endpoint paths, HTTP statuses, JSON payload shapes, and error handling behavior.
- Keep the worker on the existing stdlib HTTP server.

**Non-Goals:**

- Do not introduce FastAPI, Flask, Starlette, Pydantic, or another HTTP framework.
- Do not change `JobManager`, provisioning behavior, schema validation rules, or request/response contracts.
- Do not add new endpoints or remove existing endpoints.
- Do not change bearer-token authorization semantics.

## Decisions

1. Keep `BaseHTTPRequestHandler` and refactor internally.

   The worker has only `GET /status`, `POST /start`, and `POST /cancel`. A web framework would make route declarations more familiar, but it would add runtime dependencies, container attack surface, and a new framework lifecycle for a very small API. Keeping stdlib preserves the current deployment profile while still allowing cleaner code.

2. Introduce explicit route handler methods.

   `do_GET` and `do_POST` should become thin dispatch entry points. Endpoint behavior should live in named methods such as `_handle_status`, `_handle_start`, and `_handle_cancel`. This makes the file read like a small router/controller while keeping request handling behavior centralized.

3. Centralize shared request handling.

   Authorization, JSON body reading, `WorkerError` mapping, and JSON response writing should remain shared code paths. This reduces duplication and helps preserve existing behavior for malformed requests, unauthorized requests, oversized bodies, and unknown endpoints.

4. Keep tests behavior-oriented.

   Existing API tests already cover the external contract. The implementation should keep those tests green and add or adjust focused tests only if the refactor introduces a new branch that is not currently covered.

## Risks / Trade-offs

- Refactor accidentally changes error precedence or status codes -> Keep the existing API test suite as the primary guard and avoid changing error classes or schema parsers.
- A custom dispatch helper could become over-engineered -> Use simple dictionaries or direct helper methods only; avoid a general routing abstraction beyond the current endpoint set.
- Internal readability improves less than a framework would -> Accept this trade-off because the worker's dependency-light container profile is more valuable for a three-endpoint API.

## Migration Plan

No data migration or deployment coordination is required. Ship the refactor with the existing provisioner worker tests. Rollback is a normal code revert because the public API and persisted state are unchanged.

## Open Questions

None.
