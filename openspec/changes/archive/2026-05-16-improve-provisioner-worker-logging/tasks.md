## 1. Command Runner Logging

- [x] 1.1 Update `CommandRunner.run()` so subprocess stdout and stderr reach the worker process console instead of being discarded.
- [x] 1.2 Preserve the existing polling loop for cancellation, timeout enforcement, process termination, and non-zero exit mapping.
- [x] 1.3 Ensure the logging behavior remains provider-agnostic and does not add RunPod-specific log handling.

## 2. Worker Status Safety

- [x] 2.1 Confirm dependency installation still reports the same stable worker phase and progress values through `GET /status`.
- [x] 2.2 Ensure raw subprocess output is not copied into `diagnostic_message`, worker error payloads, native command responses, or persisted workspace state.
- [x] 2.3 Preserve existing structured failure codes for dependency installation, timeouts, cancellation, and unexpected errors.

## 3. Tests

- [x] 3.1 Add or update command runner tests proving subprocess stdout and stderr are emitted to the worker console.
- [x] 3.2 Add or update worker API tests proving raw subprocess output remains absent from `/status` while a job is running and after command failure.
- [x] 3.3 Run the provisioner worker test suite with `PYTHONPATH=src python -m unittest discover -s tests`.

## 4. Verification

- [x] 4.1 Run `PYTHONPATH=src python -m compileall src tests` in `workers/provisioner`.
- [x] 4.2 Run `openspec status --change "improve-provisioner-worker-logging"` and confirm the change is apply-ready.
