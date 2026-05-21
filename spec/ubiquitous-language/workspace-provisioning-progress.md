## Workspace Provisioning Progress

A Native Layer process/progress object returned to Client (React) while Workspace Provisioning is idle, running, cancelling, completed, or failed.

Workspace Provisioning Progress is derived from authoritative Workspace metadata, Provider Resource snapshots, Provider observations, and Provisioner Worker progress.

Concurrent sync calls may return the latest persisted Workspace Provisioning Progress without including the result of an active in-flight sync.

Workspace Provisioning Progress includes:

- status
- phase when concrete provisioning work is active
- percent when a meaningful percentage is available
- structured failure detail when provisioning reached a durable failed Workspace state

**Invariants:**

- Workspace Provisioning Progress is not authoritative durable state; Workspace metadata is authoritative.
- Client (React) may use Workspace Provisioning Progress for rendering and for deciding whether to continue calling provisioning sync.
- Client (React) must not use Workspace Provisioning Progress to decide the next provider mutation.
- Client (React) must not parse free-form messages to classify provisioning failures.
- Native Layer must not include Provider API Keys, worker tokens, raw provider responses, stack traces, command output, or other secrets in failure metadata.
- Status `idle` means provisioning is not active for the Workspace.
- Status `running` means Native Layer is performing or resuming provisioning work; phase should identify the concrete current step when known.
- Status `cancelling` means Native Layer is cleaning up provider resources after user cancellation.
- Status `completed` must correspond to Workspace lifecycle `Ready`.
- Status `failed` must correspond to Workspace lifecycle `Failed`.
- Command failures that do not change durable Workspace state remain `NativeCommandError` responses rather than failed progress.
- Phase uses terminal values such as `not_started`, `completed`, and `failed` when no active concrete phase applies.
- Phase is a rendering and sync-loop hint; it does not authorize provider mutation.
- `failure` is populated for failed progress and is derived from `Workspace.last_provisioning_failure`.
- Failed legacy Workspace metadata without structured failure detail is represented with a generic `legacy_failure` classification.

## See Also

- [Workspace](./workspace.md)
- [Workspace Provisioning](../flows/workspace-provisioning.md)
- [Provisioner Worker](./provisioner-worker.md)
- [Provider Resource](./provider-resource.md)
