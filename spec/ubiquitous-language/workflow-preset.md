## Workflow Preset

A bundled application-level definition of a supported generation workflow.

A Workflow Preset includes:

- workflow execution type (eg t2i, i2i, t2v, etc.)
- list of required models and checkpoints
- reference to a ComfyUI Workflow
- required ComfyUI version
- required base Persistent Storage Volume size

**Invariants:**

- The Workflow Preset explicitly declares the required base Persistent Storage Volume size for required models, checkpoints, assets, and workflow files.
- The Workflow Preset is not created or modified by the user in v1.
- Runtime extensions are not described by Workflow Presets in v1; bundled workflows that need extensions rely on the selected runtime image implementation.

## See Also

- [Workflow](./workflow.md)
- [Persistent Storage Volume](./persistent-storage-volume.md)
