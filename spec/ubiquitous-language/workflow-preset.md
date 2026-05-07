## Workflow Preset

A bundled application-level definition of a supported generation workflow.

A Workflow Preset includes:

- workflow execution type (eg t2i, i2i, t2v, etc.)
- list of required models and checkpoints
- reference to a ComfyUI Workflow
- required ComfyUI version
- optional required Custom Nodes list
- required base Persistent Storage Volume size

**Invariants:**

- The Workflow Preset explicitly declares the required base Persistent Storage Volume size for required models, checkpoints, assets, ComfyUI and custom nodes dependencies, and workflow files.
- The Workflow Preset is not created or modified by the user in v1.
- Custom Nodes are optional: a Workflow Preset may require none.

## See Also

- [Workflow](./workflow.md)
- [Custom Nodes](./custom-nodes.md)
- [Endpoint Profile](./endpoint-profile.md)
- [Persistent Storage Volume](./persistent-storage-volume.md)
