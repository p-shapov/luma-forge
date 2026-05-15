## 1. Refactor HTTP Adapter

- [x] 1.1 Extract named route handlers for `GET /status`, `POST /start`, and `POST /cancel` in `workers/provisioner/src/provisioner_worker/api.py`.
- [x] 1.2 Introduce a small shared dispatch path that performs authorization, endpoint lookup, JSON body reading where needed, worker error mapping, and JSON response writing.
- [x] 1.3 Preserve the existing request validation order, status codes, response payload shapes, and token-safe error behavior.
- [x] 1.4 Keep the implementation on Python stdlib `http.server` without adding a web framework dependency.

## 2. Verification

- [x] 2.1 Run the provisioner API test suite and fix any behavior regressions.
- [x] 2.2 Run the provisioner syntax check.
- [x] 2.3 Review `workers/provisioner/pyproject.toml` to confirm no new HTTP framework dependency was added.
