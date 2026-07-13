# Bundled Catalog Cutover Design

## Goal

Make the revision-based catalog currently stored under `new_bundled/catalog`
the only bundled catalog used by the native application, worker image builds,
and worker image promotion workflows.

The final repository has one root named `bundled`. The old flat files
`workflow-catalog.json`, `execution-schemas.json`, `runtime-contracts.json`,
`runtime-presets/*.json`, and `workflows/*.json` are deleted. No compatibility
reader, generated legacy registry, fallback path, or deprecated contract is
retained.

## Chosen Approach

Use the revision-based filesystem layout directly everywhere.

This is preferable to generating the old aggregate JSON files because it keeps
one physical and logical source of truth. It is also preferable to a native-only
rename because the current RunPod endpoint and provisioner release pipelines
still read and mutate the old flat catalog.

## Final Layout and Packaging

Rename `new_bundled` to `bundled` after deleting the old `bundled` directory.
The resulting source layout starts at:

```text
bundled/
└── catalog/
    ├── contracts/
    ├── schemas/
    └── entries/
        ├── execution_schemas/<id>/<revision>/execution_schema
        ├── runtime_contracts/<id>/<revision>/runtime_contract
        ├── runtime_presets/<id>/<revision>/runtime_preset
        └── workflows/<id>/<revision>/
            ├── metadata
            ├── model_assets
            ├── contract_requirements
            ├── execution_contract
            └── workflow
```

`src-tauri/tauri.conf.json` continues to map the repository source
`../bundled/` to the packaged resource `bundled/`. Native startup continues to
resolve `resource_dir/bundled`. Rust bundled type generation and the packaged
catalog smoke test change their repository source path from `new_bundled` to
`bundled`.

## Worker Catalog Reads

Worker tooling addresses immutable entries by `(id, revision)` and derives the
physical path from the catalog root. IDs and revisions must remain safe single
path components, and referenced documents must exist.

For a RunPod endpoint build, the release tool:

1. opens the selected workflow revision;
2. reads its runtime preset reference from `metadata`;
3. reads its execution schema reference from `execution_contract`;
4. loads the referenced runtime preset and execution schema revisions;
5. returns the concrete workflow, execution contract, execution schema, and
   runtime preset paths together with the existing image build metadata.

The endpoint Docker build copies those concrete files. Its metadata tool merges
the selected workflow `execution_contract` document with the referenced
`execution_schema` into the runtime execution contract consumed by the worker.
It no longer copies or parses aggregate catalog registries.

The provisioner release tool reads runtime contract revision directories
directly. Both release tools derive the next contract patch revision from the
existing revision directory names.

## Immutable Promotion

Catalog revision directories are immutable. Promotion never overwrites an
existing revision document.

Endpoint image promotion performs these writes:

1. create
   `bundled/catalog/entries/runtime_contracts/<endpoint-id>/<next-patch>/runtime_contract`
   with the digest-pinned `image_ref`;
2. copy the selected workflow revision into the workflow's next available patch
   revision directory;
3. update only the new revision's RunPod `endpoint_contract_ref.revision`.

Provisioner image promotion performs the same runtime-contract write for the
`provisioner` contract. For every workflow whose latest revision references that
contract, it creates the workflow's next patch revision and updates only the new
revision's `provisioner_contract_ref.revision`.

Promotion rejects mutable image references, unsafe identifiers, malformed
semantic versions, duplicate destination revisions, missing source revisions,
and missing or mismatched contract references before writing catalog files.
The tools write the new runtime contract and workflow revision only after all
inputs and destination paths have been validated.

## GitHub Workflow Changes

The deployment workflows pass `bundled/catalog` plus logical IDs and revisions
to the release tools instead of paths to aggregate registries.

Promotion PR scope checks accept only:

- the newly created runtime contract revision directory; and
- the newly created workflow revision directories.

The pull request action stages those revision paths. PR descriptions state that
promotion creates new immutable catalog revisions rather than updating a
Workflow Preset in place.

## Tests and Documentation

Rust verification covers the renamed packaged catalog source and unchanged
runtime resource destination. Worker unit tests use small temporary
revision-based catalog trees; one repository-backed smoke path may use the
packaged `bundled/catalog` data. Dockerfile and GitHub workflow assertions target
the revision paths and reject the removed flat paths.

Current worker READMEs describe the revision-based build and promotion flow.
Historical implementation plans may retain their original path names, but live
code, tests, CI configuration, and current operational documentation must not
reference `new_bundled` or any deleted flat catalog file.

Final verification includes:

- native Rust tests, formatting, and strict Clippy;
- RunPod endpoint and provisioner Python test suites;
- native command code generation and frontend build if affected by the branch;
- a focused search proving live paths contain no `new_bundled`,
  `workflow-catalog.json`, `execution-schemas.json`, or
  `runtime-contracts.json` references.

## Non-Goals

- Preserving the old catalog shape or filenames.
- Adding a catalog cache, database mirror, or catalog generation step.
- Changing application-layer catalog models or the packaged resource name.
- Changing worker runtime behavior beyond how build inputs and promoted image
  references are resolved.
