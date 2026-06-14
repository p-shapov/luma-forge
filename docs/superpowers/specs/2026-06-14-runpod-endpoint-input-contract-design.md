# RunPod Endpoint Input Contract Design

## Context

The current RunPod Endpoint Worker accepts a flat payload with a single
`prompt` field and hard-codes HiDream O1 Dev ComfyUI node mutations. LumaForge
will support more ComfyUI workflows later, so public request semantics and
workflow-specific injection rules need explicit contracts.

Workflow presets already define durable product-level revisions in
`bundled/workflow-catalog.json`. Runtime contracts pin worker image revisions in
`bundled/runtime-contracts.json`. Public execution schemas should be reusable
across workflow presets, while each workflow revision should own the mapping
from those schema fields into its baked ComfyUI workflow.

## Scope

This design defines the contract shape only. Implementation will follow in a
separate plan after review.

The contract has two layers:

- a bundled execution schema registry with reusable public input and output
  schemas
- a per-workflow-revision execution contract that references one schema and
  maps schema values or constants into ComfyUI node paths

The public RunPod endpoint request remains flat. The endpoint image should
contain the baked ComfyUI workflow plus the matching workflow revision execution
contract and referenced execution schema.

## Goals

- Define public request and response semantics outside worker code.
- Keep workflow-specific ComfyUI node knowledge in workflow revisions.
- Allow multiple workflow presets to share one schema, such as `text-to-image`.
- Keep endpoint request payloads flat, for example `{ "prompt": "..." }`.
- Keep output schemas runtime-agnostic.
- Make the worker generic enough to apply revision bindings without
  HiDream-specific code.
- Keep secrets out of schemas, bindings, request payloads, logs, error payloads,
  and test fixtures.

## Non-Goals

- No compatibility layer for the current hard-coded HiDream patcher.
- No UI form generation in this contract.
- No secret-valued workflow inputs.
- No request-carried ComfyUI node ids, paths, or binding instructions.
- No runtime-specific output storage details in execution schemas.
- No legacy tests that assert removed hard-coded HiDream behavior.

## Execution Schema Registry

A new bundled registry defines reusable execution schemas. The first schema is
`text-to-image`.

Example shape:

```json
{
  "execution_schemas": [
    {
      "id": "text-to-image",
      "revisions": [
        {
          "version": "1.0.0",
          "inputs": [
            {
              "id": "prompt",
              "type": "string",
              "required": true,
              "max_length": 4000
            }
          ],
          "outputs": {
            "type": "image_set"
          }
        }
      ]
    }
  ]
}
```

`outputs.type` describes the logical result shape only. Runtime-specific storage
details, such as RunPod volume artifact references, belong to runtime response
handling and must not be part of the reusable schema.

Future schemas can add other workflow families, for example `image-to-image` or
`text-to-video`, only when a workflow revision needs them.

## Workflow Revision Execution Contract

Each workflow revision references one execution schema and defines ordered
bindings into the baked ComfyUI workflow.

Example HiDream O1 Dev revision fragment:

```json
{
  "version": "1.0.0",
  "runtime_preset": "comfyui-py312-cu126-torch291",
  "execution_contract": {
    "schema_ref": {
      "id": "text-to-image",
      "version": "1.0.0"
    },
    "input_bindings": [
      {
        "value": "{{prompt}}",
        "node_id": "171",
        "path": ["widgets_values", "0"]
      },
      {
        "value": false,
        "node_id": "154",
        "path": ["widgets_values", "0"]
      },
      {
        "value": false,
        "node_id": "177",
        "path": ["widgets_values", "0"]
      }
    ]
  }
}
```

`schema_ref` is versioned because schemas are shared contracts. A workflow
revision can update its bindings without changing the schema, or move to a new
schema version when public request or output semantics change.

## Endpoint Request

The public RunPod endpoint request is validated against the referenced execution
schema. For the initial `text-to-image@1.0.0` schema, the request is:

```json
{
  "prompt": "a product photo of a small lamp"
}
```

The request does not include `workflow_id`, `workflow_version`, schema metadata,
node ids, paths, or binding instructions. A deployed workspace is already pinned
to a workflow revision, so the endpoint worker validates requests against the
baked schema and execution contract for that revision.

## Input Binding Semantics

`execution_contract.input_bindings` is an ordered list of workflow mutations.

A binding has:

- `value`: either a template string or a literal JSON value
- `node_id`: the target ComfyUI workflow node id, serialized as a string
- `path`: path segments relative to the selected node object

If `value` is a string exactly matching `{{field_id}}`, the worker substitutes
the validated request value for `field_id`. `field_id` must exist in the
referenced execution schema.

Any other `value` is treated as a literal constant. This includes non-string
JSON values such as `false`, numbers, arrays, and objects, plus ordinary strings
that do not exactly match the template syntax.

For the current HiDream O1 Dev revision:

- `{{prompt}}` injects the request prompt into node `171` at
  `widgets_values[0]`.
- `false` patches node `154` at `widgets_values[0]` to disable image edit.
- `false` patches node `177` at `widgets_values[0]` to disable prompt refine.

`path` values are strings in catalog JSON. When applying a path to an array, a
numeric string segment is interpreted as an array index.

## Validation

Schema registry validation should reject:

- duplicate schema ids or duplicate versions for one schema id
- missing or empty schema id or revisions
- missing or empty revision version, inputs, or outputs
- unknown input types
- invalid input constraints, such as a zero `max_length`
- missing or empty `outputs.type`
- secret-like input ids

Workflow catalog validation should reject:

- missing `execution_contract`
- missing or unknown `schema_ref`
- empty `input_bindings`
- bindings without `value`, `node_id`, or `path`
- malformed template strings
- template references not defined by the referenced schema
- missing bindings for required schema fields
- empty `node_id`
- empty `path`

Request validation should reject:

- non-object input payloads
- missing required fields for the selected schema
- unknown request fields
- field values that fail selected schema constraints

Workflow application should fail fast when:

- a target node is absent
- a path segment is absent
- a numeric array index is out of range
- a path expects an array but the target value is not an array

## Worker Boundary

Endpoint deployment should resolve the selected workflow revision and bake
image-local runtime files for:

- the selected ComfyUI workflow JSON
- the selected workflow revision execution contract
- the referenced execution schema revision

The worker should load only those image-local files at request time. It should
not fetch workflow catalog or execution schema registry data from the app,
RunPod, or another external source while handling a generation request.

The worker should not hard-code HiDream-specific node ids, prompt fields, or
constant patches. It validates the request against the baked execution schema
revision, applies the baked execution contract bindings to a deep copy of the
workflow, runs ComfyUI, and returns UI-safe success and failure payloads using
the existing failure safety rules. The worker should fail during request
handling if any required runtime file is absent or invalid.

## Testing

Implementation tests should cover:

- execution schema registry decoding and validation
- workflow catalog decoding and validation for `schema_ref` and
  `input_bindings`
- request validation for `text-to-image@1.0.0`
- template substitution with `{{prompt}}`
- literal constant injection
- ordinary string constants that are not templates
- failures for missing nodes and invalid paths
- rejection of unknown request fields
- rejection of secret-like schema input ids

## Verification

When implementation begins, run the relevant catalog and endpoint worker tests:

```bash
cargo test --manifest-path src-tauri/Cargo.toml workflow_catalog
PYTHONPATH=workers/runpod-endpoint/src python3 -m unittest discover -s workers/runpod-endpoint/tests
```
