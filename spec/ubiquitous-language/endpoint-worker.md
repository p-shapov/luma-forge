## Endpoint Worker

A worker process running behind the Serverless Endpoint.

The Endpoint Worker is responsible for the runtime contract between the Serverless Endpoint and the prepared ComfyUI environment.

The Endpoint Worker assumes that the required ComfyUI instance, Workflow definition, models, assets, dependencies, and custom nodes are already available in the prepared runtime environment.

**Invariants:**

- The Endpoint Worker image reference is supplied by Native build-time configuration.
- The Endpoint Worker runs behind a Serverless Endpoint.
- The Endpoint Worker communicates with a prepared ComfyUI instance.
- The Endpoint Worker defines the runtime API contract used by the desktop application for generation.
- The Endpoint Worker must not perform provisioning or ComfyUI environment setup.

## See Also

- [Serverless Endpoint](./serverless-endpoint.md)
- [GPU Cloud Provider](./gpu-cloud-provider.md)
- [Workflow Preset](./workflow-preset.md)
- [Workflow](./workflow.md)
