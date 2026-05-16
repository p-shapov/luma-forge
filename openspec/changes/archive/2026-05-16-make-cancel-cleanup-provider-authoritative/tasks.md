## 1. Cleanup Semantics

- [x] 1.1 Update destructive Workspace Provisioning cancellation cleanup so it does not call worker `/cancel`.
- [x] 1.2 Ensure provider resources are still deleted in dependency-safe order: Serverless Endpoint, endpoint template, provisioning pod, persistent volume.
- [x] 1.3 Preserve existing tolerance for already-missing provider resources.
- [x] 1.4 Preserve failure behavior when provider resource deletion or required local token cleanup fails.

## 2. Tests

- [x] 2.1 Update the cancellation test that currently expects worker cancel failure to mark the Workspace `failed`.
- [x] 2.2 Add or adjust assertions so cancellation does not invoke worker `/cancel` when active worker metadata and token exist.
- [x] 2.3 Keep coverage proving cancellation still marks the Workspace `failed` and preserves cleanup metadata when provider cleanup fails.

## 3. Verification

- [x] 3.1 Run targeted Workspace Provisioning cancellation tests.
- [x] 3.2 Run `cargo test`.
- [x] 3.3 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 3.4 Run `cargo fmt`.

## 4. Provisioner Worker API

- [x] 4.1 Remove `POST /cancel` from Provisioner Worker HTTP routing.
- [x] 4.2 Remove the worker cancel request schema and job manager entry point.
- [x] 4.3 Update worker API tests to assert `POST /cancel` is unsupported.
- [x] 4.4 Update worker README to remove the public cancel endpoint.
- [x] 4.5 Run Provisioner Worker tests.
