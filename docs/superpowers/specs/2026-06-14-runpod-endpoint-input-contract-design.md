# RunPod Endpoint Input Contract Design

## Context

The current RunPod Endpoint Worker accepts a flat payload with a single
`prompt` field and hard-codes HiDream O1 Dev ComfyUI node mutations. LumaForge
will support more ComfyUI workflows later, so the request contract and workflow
injection rules need to move out of worker-specific code and into the workflow
catalog.

Workflow presets already define durable product-level revisions in
`bundled/workflow-catalog.json`. Runtime contracts pin worker image revisions in
`bundled/runtime-contracts.json`. Input contracts belong to workflow revisions
because they describe workflow semantics, not just container images.

## Scope

The catalog contract shape is already defined by the workflow revision
`schema.type` and `schema.input_bindings` fields. The implementation work is to
make the RunPod endpoint worker consume that contract instead of using
HiDream-specific code.

The planned implementation scope is:

- Decode and validate the revision-level `schema` contract from catalog-derived
  metadata.
- Implement the reusable `text-to-image` schema that the worker uses to validate
  requests.
- Replace hard-coded HiDream node mutation logic with a generic input binding
  engine.
- Keep the public endpoint request flat, for example `{ "prompt": "..." }`.
- Bake both the ComfyUI workflow file and its revision schema metadata into the
  endpoint Docker image.
- Keep existing UI-safe success and failure response contracts.

## Goals

- Define the public input contract for each workflow revision.
- Keep the RunPod endpoint request payload flat.
- Support reusable workflow-family schema types such as `text-to-image`.
- Define how request fields and preset constants are injected into a baked
  ComfyUI workflow.
- Keep the RunPod endpoint worker generic enough to execute multiple workflow
  contracts.
- Make the Docker image contain all workflow-specific runtime metadata needed by
  the worker.
- Keep secrets out of request schemas, bindings, logs, error payloads, and test
  fixtures.

## Non-Goals

- No compatibility layer for the current hard-coded HiDream patcher.
- No UI metadata in the worker contract.
- No support for secret-valued workflow inputs.
- No runtime catalog ownership of workflow input semantics.
- No frontend form generation in this implementation scope.

## Catalog Shape

Each RunPod endpoint-capable workflow revision defines a `schema` object
directly:

```json
{
  "version": "1.0.0",
  "runtime_preset": "comfyui-py312-cu126-torch291",
  "schema": {
    "type": "text-to-image",
    "input_bindings": [
      {
        "value": "{prompt}",
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
  },
  "requires_hugging_face_api_key": true,
  "required_volume_size_gb": 19
}
```

The workflow revision version is the version boundary for this contract. There
is no nested input contract version.

## Schema Types

`schema.type` selects a reusable workflow-family schema. The initial schema type
is `text-to-image`.

The `text-to-image` schema defines one public field:

- `prompt`: required non-empty string, maximum 4000 characters.

Future schema types can define other workflow families, for example
`image-to-image` or `video-to-video`. Those schema types should be added only
when a workflow revision needs them.

## Endpoint Request

The public RunPod endpoint request remains flat:

```json
{
  "prompt": "a product photo of a small lamp"
}
```

The request does not include `workflow_id`, `workflow_version`, or another
schema envelope. A deployed workspace is already pinned to a workflow revision,
so the RunPod endpoint worker should validate requests against the baked
revision contract.

## Input Bindings

`schema.input_bindings` is the ordered list of workflow injections for the
revision.

A binding has:

- `value`: either a placeholder string or a literal JSON value.
- `node_id`: the target ComfyUI workflow node id, serialized as a string.
- `path`: path segments relative to the selected node object.

Placeholder values use `{field_name}` syntax. The field name must exist in the
selected schema type. Literal values are preset constants and are not
user-controllable request fields.

For the current HiDream O1 Dev revision:

- `{prompt}` injects the request prompt into node `171` at
  `widgets_values[0]`.
- `false` patches node `154` at `widgets_values[0]` to disable image edit.
- `false` patches node `177` at `widgets_values[0]` to disable prompt refine.

`path` values are strings in catalog JSON. When applying a path to an array, a
numeric string segment is interpreted as an array index.

## Validation

Catalog validation should reject:

- missing `schema.type`
- unknown schema types
- empty `input_bindings`
- bindings without `value`, `node_id`, or `path`
- malformed placeholders
- placeholders not defined by the selected schema type
- missing bindings for required schema fields
- empty `node_id`
- empty `path`

Request validation should reject:

- non-object input payloads
- missing required fields for the selected schema type
- unknown request fields
- field values that fail the selected schema type constraints

Workflow application should fail fast when:

- a target node is absent
- a path segment is absent
- a numeric array index is out of range
- a path expects an array but the target value is not an array

## Worker Boundary

The RunPod endpoint worker should load the baked workflow and the baked workflow
revision schema contract from image-local runtime files. It should not hard-code
HiDream-specific node ids, prompt fields, or constant patches.

The worker applies validation first, then applies all input bindings to a copy of
the workflow. It returns UI-safe success and failure payloads using the existing
failure safety rules.

The endpoint image build should copy:

- the selected ComfyUI workflow JSON to the runtime workflow path
- the selected revision `schema` object to an image-local metadata path consumed
  by the worker

The worker should fail during request handling if either runtime file is absent
or invalid.

## Testing

Contract tests should cover:

- catalog decoding and validation for `schema.type` and `input_bindings`
- request validation for the initial `text-to-image` schema
- placeholder injection into a workflow node path
- literal constant injection into a workflow node path
- failures for missing nodes and missing paths
- rejection of unknown request fields
- rejection of secret-like schema field names before a schema can be used by the
  worker

## Verification

When implementation begins, run the relevant catalog and endpoint worker tests:

```bash
cargo test --manifest-path src-tauri/Cargo.toml workflow_catalog
PYTHONPATH=workers/runpod-endpoint/src python3 -m unittest discover -s workers/runpod-endpoint/tests
```
