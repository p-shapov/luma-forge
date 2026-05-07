## Workspace Provisioning Progress

A Native Layer process/progress object returned to Client (React) while Workspace Provisioning is idle, running, failed, completed, or cancelled.

Workspace Provisioning Progress is derived from authoritative Workspace metadata, Provider Resource snapshots, Provider observations, and Provisioner Worker progress.

Concurrent sync calls may return the latest persisted Workspace Provisioning Progress without including the result of an active in-flight sync.

Workspace Provisioning Progress includes:

- status
- phase when concrete provisioning work is active
- progress percent when a meaningful percentage is available
- diagnostic message when safe to display
- update timestamp

**Invariants:**

- Workspace Provisioning Progress is not authoritative durable state; Workspace metadata is authoritative.
- Client (React) may use Workspace Provisioning Progress for rendering and for deciding whether to continue calling provisioning sync.
- Client (React) must not use Workspace Provisioning Progress to decide the next provider mutation.
- Native Layer must not include Provider API Keys or secrets in diagnostic messages.
- Status `idle` means provisioning is not active for the Workspace.
- Status `running` means Native Layer is performing or resuming provisioning work; phase should identify the concrete current step when known.
- Status `completed` must correspond to Workspace lifecycle `Ready`.
- Status `failed` must correspond to Workspace lifecycle `Failed` or a rejected request that performed no mutation.
- Status `cancelled` means user cancellation reached a terminal result.
- Phase may be `null` when status is `idle`, `completed`, `failed`, or `cancelled`.
- Phase must not encode terminal process states such as idle, completed, failed, or cancelled.

## See Also

- [Workspace](./workspace.md)
- [Workspace Provisioning](../flows/workspace-provisioning.md)
- [Provisioner Worker](./provisioner-worker.md)
- [Provider Resource](./provider-resource.md)
