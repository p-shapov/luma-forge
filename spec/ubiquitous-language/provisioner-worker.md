## Provisioner Worker

A worker process running inside a Provisioning Pod.

The Provisioner Worker performs provisioning-time work required to prepare the remote environment for Workspace.

**Responsibilities:**

- download models
- download required ComfyUI version from the configured Git source
- download optional Custom Nodes from Git URLs
- install ComfyUI dependencies into the mounted workspace virtual environment
- install Custom Nodes dependencies into the mounted workspace virtual environment
- report provisioning progress to the desktop application

**Invariants:**

- The Provisioner Worker runs inside a Provisioning Pod.
- The Provisioner Worker image reference is supplied by Native build-time configuration.
- The Provisioner Worker prepares the remote environment for Workspace.
- The Provisioner Worker must install ComfyUI runtime dependencies into the mounted network volume, not the ephemeral container filesystem.
- The Provisioner Worker reports provisioning progress, but does not own Provider Resource lifecycle.
- In v1, models are downloaded from Hugging Face.
- In future versions, models may also be downloaded from CivitAI.
- The Provisioner Worker must not be used as the persistent runtime entry point for generation.

## See Also

- [Workspace](./workspace.md)
- [Workspace Provisioning Progress](./workspace-provisioning-progress.md)
- [Provisioning Pod](./provisioning-pod.md)
- [Custom Nodes](./custom-nodes.md)
