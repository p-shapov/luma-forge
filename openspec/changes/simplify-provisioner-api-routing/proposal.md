## Why

The provisioner worker HTTP adapter currently mixes route dispatch, authorization, JSON body handling, and response writing inside `do_GET` and `do_POST`. The API is intentionally small, so the readability issue can be addressed without introducing a web framework or changing the worker's runtime dependency profile.

## What Changes

- Refactor the provisioner worker request handler so endpoint routing is expressed through small, named route handlers.
- Preserve the existing stdlib `http.server` implementation and avoid adding FastAPI, Flask, Starlette, or similar runtime dependencies.
- Keep the public worker API unchanged: `GET /status`, `POST /start`, and `POST /cancel` continue to require bearer authorization and return the same success and error payload shapes.
- Keep validation and error mapping behavior unchanged, including request-size checks, malformed `Content-Length` handling, invalid JSON handling, unauthorized responses, and unknown endpoint responses.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `provisioner-worker`: Clarifies internal HTTP adapter structure while preserving the existing API requirements.

## Impact

- Affected code: `workers/provisioner/src/provisioner_worker/api.py`.
- Affected tests: `workers/provisioner/tests/test_api.py`.
- APIs: no external API changes.
- Dependencies: no new runtime dependencies.
