## Provisioner Worker

A worker process running inside a Provisioning Pod.

The Provisioner Worker performs provisioning-time work required to prepare the remote environment for Workspace.

**Responsibilities:**

- download models
- validate the image-baked ComfyUI runtime
- download optional Custom Nodes from Git URLs
- verify the base runtime dependencies packaged with the Provisioner Worker image
- install Custom Nodes dependencies into the mounted workspace virtual environment
- report provisioning progress to the desktop application

**Invariants:**

- The Provisioner Worker runs inside a Provisioning Pod.
- The Provisioner Worker image reference is selected from the Workspace's resolved runtime implementation snapshot.
- The Provisioner Worker prepares the remote environment for Workspace.
- The Provisioner Worker must use the image-baked ComfyUI runtime and prepare only workspace-specific directories on the mounted network volume.
- The Provisioner Worker reports provisioning progress, but does not own Provider Resource lifecycle.
- In v1, models are downloaded from Hugging Face.
- In future versions, models may also be downloaded from CivitAI.
- The Provisioner Worker must not be used as the persistent runtime entry point for generation.

## See Also

- [Workspace](./workspace.md)
- [Workspace Provisioning Progress](./workspace-provisioning-progress.md)
- [Provisioning Pod](./provisioning-pod.md)
- [Custom Nodes](./custom-nodes.md)
