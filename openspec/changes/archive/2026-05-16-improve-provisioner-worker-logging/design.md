## Context

The Provisioner Worker prepares a mounted workspace volume inside a provider-started container. During ComfyUI setup it runs long-lived subprocesses such as Git and `pip install`. The worker currently reports coarse status through `GET /status`, but the command runner discards subprocess stdout/stderr. When dependency installation takes several minutes, the status endpoint remains at the last explicit progress update and provider pod logs do not show what the subprocess is doing.

This change improves operator debugging without changing the worker API contract. The worker should continue to expose only UI-safe status payloads while provider/container logs receive the raw subprocess stream.

## Goals / Non-Goals

**Goals:**

- Make long-running provisioner subprocess output visible in provider pod logs.
- Preserve existing command failure, cancellation, and timeout semantics.
- Keep `/status` payloads sanitized and stable.
- Avoid provider-specific logging integrations; rely on standard container stdout/stderr capture.
- Add focused regression tests for subprocess logging and status redaction boundaries.

**Non-Goals:**

- Add a remote log streaming API.
- Persist raw command output in workspace metadata or worker status.
- Parse `pip` output into fine-grained progress percentages.
- Add provider-specific RunPod log retrieval.
- Change dependency installation commands or install destinations.

## Decisions

1. Stream subprocess output to the worker console.

   The command runner should no longer route `run()` subprocess stdout to `DEVNULL`. It should allow stdout/stderr from commands such as `pip install` to reach the worker process console, where standard container logging can capture it. This keeps the implementation provider-agnostic because every supported provider should expose container stdout/stderr through its own logs.

   Alternative considered: capture output and return it through `/status`. Rejected because raw command output can include credential-bearing index URLs, package source URLs, environment-specific paths, or future user-provided content.

2. Preserve sanitized status responses.

   Worker status should continue to report stable phases, progress percentages, diagnostic messages, and structured error metadata. Raw command output must not be copied into `diagnostic_message`, `error.message`, or native command responses.

   Alternative considered: include the last N output lines in status after redaction. Rejected for this change because reliable redaction is non-trivial and not necessary to solve provider-side debugging.

3. Keep cancellation and timeout polling behavior.

   The existing loop around the subprocess remains responsible for cancellation and timeout enforcement. The logging change should not replace that lifecycle control with a blocking call that cannot observe cancellation promptly.

   Alternative considered: use `subprocess.run()` with inherited streams. Rejected because it would make cancellation and timeout handling less explicit.

## Risks / Trade-offs

- Raw provider logs may include package index URLs or other diagnostic details from dependency tooling. Mitigation: keep raw output only in provider/container logs and do not expose it through application APIs.
- Inherited subprocess output may be buffered by the child process or runtime. Mitigation: keep `PYTHONUNBUFFERED=1` for the worker and prefer direct stream inheritance or line-forwarding that flushes promptly.
- Tests that previously assumed quiet subprocess execution may need adjustment. Mitigation: update tests narrowly around command runner behavior.
- Provider log availability varies by provider UI/API. Mitigation: rely only on standard stdout/stderr emission and avoid RunPod-specific logic.
