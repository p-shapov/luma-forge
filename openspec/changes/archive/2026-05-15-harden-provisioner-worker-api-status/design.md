## Context

The Provisioner Worker exposes a small bearer-protected HTTP API from inside the temporary provisioning container. Its current stdlib HTTP adapter shares authorization, body parsing, routing, and JSON response writing in one request path. That works for the supported endpoints, but unsupported routes and methods can bypass parts of the worker error contract.

The worker also reports progress and terminal diagnostics through `GET /status`. Today those diagnostics include preset-provided names and identifiers. That is acceptable only while Workflow Presets are trusted catalog data. Future user-defined presets make those fields untrusted input, so worker status text must remain deterministic and UI-safe regardless of preset content.

This change is adjacent to `simplify-provisioner-api-routing`, which refactors the same HTTP adapter. The implementation should coordinate with that change by preserving the dependency-light stdlib adapter while tightening routing behavior.

## Goals / Non-Goals

**Goals:**

- Keep every worker HTTP response on the same JSON worker error payload contract.
- Return `not_found` for unsupported endpoint paths without first parsing request bodies.
- Return worker JSON errors for unsupported methods instead of stdlib HTML errors.
- Prevent unexpected preparation exceptions from emitting unsanitized tracebacks after a safe terminal error has been recorded.
- Make progress and validation diagnostics safe for future user-defined presets.
- Validate preset identifiers before they can be used as safe structured context.

**Non-Goals:**

- Add a web framework or change the worker runtime dependency profile.
- Add new public endpoints.
- Change Native-owned provisioning orchestration or provider resource cleanup behavior.
- Implement user preset import/authoring.
- Add localization or user-facing display-name formatting in the worker.

## Decisions

1. Route before reading request bodies.

   The handler should identify the method and path first. If the method/path pair is unsupported, it should reject the request with the worker JSON error payload without reading or decoding the request body. This preserves request-size protections for supported endpoints while making unsupported endpoints deterministic.

   Alternative considered: keep body parsing before routing and add special cases for invalid JSON on unknown paths. That preserves the current structure but keeps route behavior dependent on body validity, which is not useful for a three-endpoint API.

2. Add explicit unsupported-method handling.

   Implement handlers for unsupported methods, or override the method dispatch path, so the stdlib HTML `501` response is not exposed. The worker should continue to require bearer authorization before returning state or route information for API requests.

   Alternative considered: accept stdlib `501` as out of scope. That conflicts with the worker error-payload contract and makes unsupported methods behave differently from unsupported paths.

3. Treat unexpected exceptions as terminal worker failures, not thread crashes.

   `JobManager` should record a sanitized `unexpected_error` terminal snapshot and stop. It should not re-raise the original exception from the worker thread because the default thread exception handler can print raw traceback content to container stderr.

   Alternative considered: log the exception with redaction. That may be useful later, but a safe redaction layer does not exist in this worker today.

4. Keep Custom Node display names out of status messages, but allow model asset display names during download progress.

   Custom Node progress and validation failure messages should not include `node.name`. Model asset download progress may include `asset.name` because that is useful while observing large downloads. Validation failure messages should continue to use validated IDs, not display names. IDs may appear only after schema validation has constrained them to a safe bounded ASCII format.

   Alternative considered: use only IDs in all worker diagnostics. That is safer and more deterministic, but makes long-running model downloads harder to recognize.

5. Validate preset identifiers at the schema boundary.

   IDs that may be returned in structured context should be constrained to a small safe character set and bounded length. Display names should have length/control-character validation even if they are not echoed by the worker, because they may be persisted into manifests or used by future UI paths.

   Alternative considered: rely on Native to validate user presets. Native should validate too, but the worker is a security boundary for its own API and status contract.

## Risks / Trade-offs

- Existing tests may assume body validation happens before route validation. Mitigation: update tests to encode the intended contract explicitly: unsupported paths/methods are rejected before body parsing.
- Model asset progress can reflect user-defined model asset names. Mitigation: the worker validates display names for length and control characters, while failures and Custom Node diagnostics continue to use validated IDs.
- Identifier validation may reject existing ad hoc test fixtures or future imported presets. Mitigation: document the accepted identifier format and add focused tests before user preset import exists.
- There may be merge overlap with `simplify-provisioner-api-routing`. Mitigation: keep both changes aligned around the same no-framework adapter structure and resolve by preserving both readability and stricter request ordering.
