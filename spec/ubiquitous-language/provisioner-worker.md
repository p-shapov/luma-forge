## Provisioner Worker

A worker process running inside a Provisioning Pod.

The Provisioner Worker performs provisioning-time work required to prepare the remote environment for Workspace.

**Responsibilities:**

- download declared model assets
- create workspace-specific directories
- write minimal workspace-preparation metadata
- report provisioning progress to the desktop application

**Invariants:**

- The Provisioner Worker runs inside a Provisioning Pod.
- The Provisioner Worker image reference is selected from app or provider deployment configuration.
- The Provisioner Worker prepares the remote environment for Workspace.
- The Provisioner Worker must prepare only workspace-specific directories and model assets on the mounted network volume.
- The Provisioner Worker must not validate endpoint image Python, endpoint image ComfyUI root, or endpoint image runtime identity.
- The Provisioner Worker must not start ComfyUI during provisioning.
- The Provisioner Worker reports provisioning progress, but does not own Provider Resource lifecycle.
- In v1, models are downloaded from Hugging Face.
- In future versions, models may also be downloaded from CivitAI.
- The Provisioner Worker must not be used as the persistent runtime entry point for generation.

## See Also

- [Workspace](./workspace.md)
- [Workspace Provisioning Progress](./workspace-provisioning-progress.md)
- [Provisioning Pod](./provisioning-pod.md)
