## Provisioning Pod

A temporary provider-side compute resource used during provisioning.

The Provisioning Pod runs the Provisioner Worker and prepares the remote environment required by Workspace on the mounted Persistent Storage Volume.

**Invariants:**

- The Provisioning Pod is temporary and must be terminated after successful provisioning.
- Native Layer owns Provisioning Pod lifecycle orchestration.
- Client (React) must not be required to request Provisioning Pod termination after the Provisioner Worker reports terminal success.

## See Also

* [Workspace](./workspace.md)
* [Provisioner Worker](./provisioner-worker.md)
* [Placement Plan](./placement-plan.md)
