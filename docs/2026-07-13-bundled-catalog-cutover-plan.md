# Bundled Catalog Cutover Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the old flat `bundled` catalog with the revision-based catalog and make native packaging plus both worker release pipelines consume it directly.

**Architecture:** The repository and packaged application expose one `bundled/catalog` filesystem tree. Native code keeps its existing `Catalog` reader, while Python release tools address entry documents directly by safe `id/revision` components and create new immutable revision directories during promotion. Docker receives concrete source files; no aggregate registry, compatibility adapter, generator, cache, or new dependency is introduced.

**Tech Stack:** Rust/Tauri, Python 3.12 standard library, Docker, GitHub Actions YAML, JSON Schema catalog files.

## Global Constraints

- `bundled/catalog` is the only live catalog source and packaged resource.
- Delete the old flat catalog; do not preserve fallbacks or generated legacy files.
- Existing catalog revision directories are immutable.
- Worker image references remain digest-pinned `@sha256:<64 lowercase hex>` values.
- IDs are safe lowercase kebab-case path components; revisions are strict three-part semantic versions.
- Do not add dependencies or change application-layer catalog models.
- Keep tests fixture-backed, with at most one repository-packaged catalog smoke per worker/native boundary.
- Follow `src-tauri/AGENTS.md` and `workers/AGENTS.md` verification commands.

---

### Task 1: Rename the catalog and prove native packaging uses it

**Files:**
- Delete: `bundled/execution-schemas.json`
- Delete: `bundled/runtime-contracts.json`
- Delete: `bundled/runtime-presets/comfyui-py312-cu126-torch291.json`
- Delete: `bundled/workflow-catalog.json`
- Delete: `bundled/workflows/comfyui-hidream-o1-dev.json`
- Move: `new_bundled/catalog/**` to `bundled/catalog/**`
- Modify: `src-tauri/src/infra/bundled/codegen.rs`
- Modify: `src-tauri/tests/bundled_catalog.rs`
- Modify: `docs/2026-07-11-application-ports-adapters-design.md`

**Interfaces:**
- Consumes: the existing revision-based files and native `Catalog::new(root)` contract.
- Produces: repository root `bundled`, schema source `bundled/catalog/schemas`, and packaged resource destination `resource_dir/bundled`.

- [ ] **Step 1: Point the packaged catalog smoke at the final source path**

Change the real-data smoke in `src-tauri/tests/bundled_catalog.rs` to:

```rust
#[tokio::test]
async fn packaged_catalog_passes_full_audit() {
    validate(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../bundled"))
        .await
        .unwrap();
}
```

- [ ] **Step 2: Run the focused test and verify the old flat directory fails**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test bundled_catalog packaged_catalog_passes_full_audit
```

Expected: FAIL because the current `bundled` directory has no `catalog/contracts` revision layout.

- [ ] **Step 3: Replace the flat catalog with the revision tree**

Delete the five old tracked flat files/directories, move all tracked files from
`new_bundled/catalog` to `bundled/catalog`, and leave no `new_bundled` directory:

```bash
git rm bundled/execution-schemas.json bundled/runtime-contracts.json \
  bundled/runtime-presets/comfyui-py312-cu126-torch291.json \
  bundled/workflow-catalog.json \
  bundled/workflows/comfyui-hidream-o1-dev.json
git mv new_bundled bundled
```

The final tree must start with:

```text
bundled/catalog/contracts
bundled/catalog/entries
bundled/catalog/schemas
```

- [ ] **Step 4: Point Rust code generation at the renamed schema directory**

In `src-tauri/src/infra/bundled/codegen.rs`, use:

```rust
let schema_dir = repo_root.join("bundled/catalog/schemas");
```

Do not change `src-tauri/tauri.conf.json` or `src-tauri/src/lib.rs`: their
existing `../bundled/ -> bundled/` packaging and `resource_dir/bundled`
resolution are already the desired contract.

- [ ] **Step 5: Update the current architecture document path**

In `docs/2026-07-11-application-ports-adapters-design.md`, replace the current
source-context path `new_bundled/catalog/entries` with
`bundled/catalog/entries`. Do not rewrite historical implementation plans.

- [ ] **Step 6: Run native catalog verification**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test bundled_catalog
cargo test --manifest-path src-tauri/Cargo.toml --lib infra::bundled
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: all commands PASS.

- [ ] **Step 7: Audit the packaged mapping**

Run:

```bash
rg -n '"../bundled/"|BUNDLED_DIR_NAME|bundled/catalog/schemas|\.\./bundled' \
  src-tauri/tauri.conf.json src-tauri/src/lib.rs \
  src-tauri/src/infra/bundled/codegen.rs src-tauri/tests/bundled_catalog.rs
test ! -e new_bundled
```

Expected: source paths all point at `bundled`, packaged destination remains
`bundled`, and `new_bundled` does not exist.

- [ ] **Step 8: Commit the catalog cutover**

```bash
git add -A -- bundled new_bundled src-tauri/src/infra/bundled/codegen.rs \
  src-tauri/tests/bundled_catalog.rs \
  docs/2026-07-11-application-ports-adapters-design.md
git commit -m "refactor(bundled): promote revision catalog"
```

---

### Task 2: Build RunPod endpoint images from direct revision documents

**Files:**
- Modify: `workers/runpod-endpoint/release_tool.py`
- Modify: `workers/runpod-endpoint/src/tools/build_metadata.py`
- Modify: `workers/runpod-endpoint/Dockerfile`
- Modify: `workers/runpod-endpoint/tests/test_release_tool.py`
- Modify: `workers/runpod-endpoint/tests/test_build_metadata.py`
- Modify: `workers/runpod-endpoint/tests/test_workflow_bindings.py`

**Interfaces:**
- Consumes: `bundled/catalog`, workflow `(id, revision)`, and the catalog reference documents.
- Produces: `resolve` outputs named `workflow_path`, `execution_contract_path`, `execution_schema_path`, `runtime_preset_path`, `workflow_id`, `workflow_revision`, `contract_id`, `contract_revision`, and the existing Python/ComfyUI/PyTorch build metadata.
- Produces: `extract_runtime_metadata(execution_contract_path: Path, execution_schema_path: Path, execution_contract_output_path: Path) -> None`.

- [ ] **Step 1: Replace aggregate-registry tests with direct-entry tests**

In `workers/runpod-endpoint/tests/test_release_tool.py`, set:

```python
CATALOG_ROOT = ROOT / "bundled/catalog"
WORKFLOW_ID = "comfyui-hidream-o1-dev"
WORKFLOW_REVISION = "1.0.0"
```

Keep one repository-backed resolve smoke:

```python
def test_resolve_outputs_direct_catalog_documents(self):
    outputs = release_tool.resolve_endpoint_build(
        catalog_root=CATALOG_ROOT,
        workflow_id=WORKFLOW_ID,
        workflow_revision=WORKFLOW_REVISION,
    )

    self.assertEqual(
        str(CATALOG_ROOT / "entries/workflows" / WORKFLOW_ID / WORKFLOW_REVISION / "workflow"),
        outputs["workflow_path"],
    )
    self.assertEqual(
        str(CATALOG_ROOT / "entries/workflows" / WORKFLOW_ID / WORKFLOW_REVISION / "execution_contract"),
        outputs["execution_contract_path"],
    )
    self.assertEqual(
        str(CATALOG_ROOT / "entries/execution_schemas/text-to-image/1.0.0/execution_schema"),
        outputs["execution_schema_path"],
    )
    self.assertEqual("1.0.1", outputs["contract_revision"])
    self.assertEqual("3.12", outputs["runtime_python_version"])
```

Add one trust-boundary check:

```python
def test_resolve_rejects_unsafe_workflow_id(self):
    with self.assertRaisesRegex(release_tool.ReleaseToolError, "invalid workflow id"):
        release_tool.resolve_endpoint_build(
            catalog_root=CATALOG_ROOT,
            workflow_id="../workflow",
            workflow_revision="1.0.0",
        )
```

Delete tests and fixture builders that exercise `workflow_presets`, `contracts`,
or `execution_schemas` aggregate objects.

- [ ] **Step 2: Write the failing direct build-metadata test**

Replace `workers/runpod-endpoint/tests/test_build_metadata.py` with a temporary
two-document test:

```python
def test_extracts_execution_contract_from_direct_documents(self):
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        source_contract = root / "execution_contract"
        source_schema = root / "execution_schema"
        output = root / "runtime_execution_contract.json"
        source_contract.write_text(json.dumps({
            "schema_ref": {
                "contract": "catalog/contracts/execution_schema_revision",
                "id": "text-to-image",
                "revision": "1.0.0",
            },
            "input_bindings": [
                {"value": "{{prompt}}", "node_id": "171", "path": ["widgets_values", "0"]}
            ],
        }), encoding="utf-8")
        source_schema.write_text(json.dumps({
            "inputs": [{"id": "prompt", "type": "string", "required": True, "max_length": 4000}],
            "outputs": {"type": "image_set"},
        }), encoding="utf-8")

        extract_runtime_metadata(
            execution_contract_path=source_contract,
            execution_schema_path=source_schema,
            execution_contract_output_path=output,
        )

        value = json.loads(output.read_text(encoding="utf-8"))
        self.assertEqual("image_set", value["execution_schema"]["outputs"]["type"])
        self.assertEqual("{{prompt}}", value["input_bindings"][0]["value"])
        self.assertNotIn("schema_ref", value)
```

- [ ] **Step 3: Run the focused worker tests and verify they fail**

```bash
PYTHONPATH=workers/runpod-endpoint/src python3 -m unittest discover \
  -s workers/runpod-endpoint/tests -p 'test_build_metadata.py'
PYTHONPATH=workers/runpod-endpoint/src python3 -m unittest discover \
  -s workers/runpod-endpoint/tests -p 'test_release_tool.py'
```

Expected: FAIL because the new function signatures and filesystem resolver do
not exist.

- [ ] **Step 4: Implement direct entry resolution with standard-library paths**

In `workers/runpod-endpoint/release_tool.py`, replace aggregate catalog lookup
helpers with these boundaries and keep the existing JSON/YAML scalar validation
helpers only where still used:

```python
ENTRY_ROOTS = {
    "execution_schema": "execution_schemas",
    "runtime_contract": "runtime_contracts",
    "runtime_preset": "runtime_presets",
    "workflow": "workflows",
}


def entry_file(
    catalog_root: Path,
    kind: str,
    entry_id: str,
    revision: str,
    filename: str,
) -> Path:
    if kind not in ENTRY_ROOTS:
        raise ReleaseToolError(f"unsupported catalog entry kind: {kind}")
    if not _is_safe_identifier(entry_id):
        raise ReleaseToolError(f"invalid {kind.replace('_', ' ')} id")
    _parse_semver(revision)
    path = catalog_root / "entries" / ENTRY_ROOTS[kind] / entry_id / revision / filename
    if not path.is_file():
        raise ReleaseToolError(f"catalog entry file does not exist: {path}")
    return path


def catalog_ref(value: dict[str, Any], key: str, contract: str) -> tuple[str, str]:
    reference = _dict_value(value, key)
    if _string_value(reference, "contract") != contract:
        raise ReleaseToolError(f"{key} uses an unexpected contract")
    entry_id = _string_value(reference, "id")
    revision = _string_value(reference, "revision")
    if not _is_safe_identifier(entry_id):
        raise ReleaseToolError(f"invalid {key} id")
    _parse_semver(revision)
    return entry_id, revision


def next_revision(entries_root: Path, *, initial: str | None = None) -> str:
    if not entries_root.is_dir():
        if initial is not None:
            _parse_semver(initial)
            return initial
        raise ReleaseToolError(f"catalog entry has no revisions: {entries_root}")
    revisions = [_parse_semver(path.name) for path in entries_root.iterdir() if path.is_dir()]
    if not revisions:
        if initial is not None:
            _parse_semver(initial)
            return initial
        raise ReleaseToolError(f"catalog entry has no revisions: {entries_root}")
    major, minor, patch = max(revisions)
    return _format_semver((major, minor, patch + 1))


def resolve_endpoint_build(
    *, catalog_root: Path, workflow_id: str, workflow_revision: str
) -> dict[str, str]:
    workflow_dir = entry_file(
        catalog_root, "workflow", workflow_id, workflow_revision, "workflow"
    ).parent
    metadata = _load_json(workflow_dir / "metadata")
    execution_contract_path = workflow_dir / "execution_contract"
    execution_contract = _load_json(execution_contract_path)

    preset_id, preset_revision = catalog_ref(
        metadata,
        "runtime_preset_ref",
        "catalog/contracts/runtime_preset_revision",
    )
    schema_id, schema_revision = catalog_ref(
        execution_contract,
        "schema_ref",
        "catalog/contracts/execution_schema_revision",
    )
    runtime_preset_path = entry_file(
        catalog_root, "runtime_preset", preset_id, preset_revision, "runtime_preset"
    )
    execution_schema_path = entry_file(
        catalog_root, "execution_schema", schema_id, schema_revision, "execution_schema"
    )
    runtime = _dict_value(_load_json(runtime_preset_path), "runtime")
    pytorch = _dict_value(runtime, "pytorch")
    contract_id = endpoint_contract_id(workflow_id)
    contract_root = catalog_root / "entries/runtime_contracts" / contract_id

    return {
        "workflow_path": str(workflow_dir / "workflow"),
        "execution_contract_path": str(execution_contract_path),
        "execution_schema_path": str(execution_schema_path),
        "runtime_preset_path": str(runtime_preset_path),
        "workflow_id": workflow_id,
        "workflow_revision": workflow_revision,
        "contract_id": contract_id,
        "contract_revision": next_revision(contract_root, initial="1.0.0"),
        "runtime_python_version": _string_value(runtime, "python_version"),
        "comfyui_revision": _string_value(runtime, "comfyui_revision"),
        "pytorch_index_url": _string_value(pytorch, "index_url"),
        "pytorch_packages_json": json.dumps(_string_list_value(pytorch, "packages"), separators=(",", ":")),
    }
```

Validate the ComfyUI revision with the existing 40-lowercase-hex check. Change
the `resolve` CLI to accept `--catalog-root`, `--workflow-id`,
`--workflow-revision`, and optional `--github-output`. Remove
`--workflow-catalog`, `--runtime-presets-dir`, and `--catalog`.

- [ ] **Step 5: Simplify build metadata to two source documents**

Replace `extract_runtime_metadata` in
`workers/runpod-endpoint/src/tools/build_metadata.py` with:

```python
def extract_runtime_metadata(
    *,
    execution_contract_path: Path,
    execution_schema_path: Path,
    execution_contract_output_path: Path,
) -> None:
    source = _load_json(execution_contract_path)
    output = {
        "execution_schema": _load_json(execution_schema_path),
        "input_bindings": _list_value(source, "input_bindings"),
    }
    execution_contract_output_path.parent.mkdir(parents=True, exist_ok=True)
    _write_json(execution_contract_output_path, output)
```

Change its CLI flags to `--execution-contract`, `--execution-schema`, and
`--execution-contract-output`. Delete aggregate lookup helpers that become
unused.

- [ ] **Step 6: Pass direct files into the Docker build**

In `workers/runpod-endpoint/Dockerfile`, replace catalog registry arguments and
copies with:

```dockerfile
ARG LUMA_FORGE_WORKFLOW_PATH
ARG LUMA_FORGE_EXECUTION_CONTRACT_PATH
ARG LUMA_FORGE_EXECUTION_SCHEMA_PATH

WORKDIR /runtime-build
COPY ${LUMA_FORGE_WORKFLOW_PATH} ./workflow.json
COPY ${LUMA_FORGE_EXECUTION_CONTRACT_PATH} ./catalog-execution-contract.json
COPY ${LUMA_FORGE_EXECUTION_SCHEMA_PATH} ./execution-schema.json
COPY workers/runpod-endpoint/src/tools/build_metadata.py ./extract_runtime_metadata.py

RUN test -n "$LUMA_FORGE_WORKFLOW_PATH" \
    && test -n "$LUMA_FORGE_EXECUTION_CONTRACT_PATH" \
    && test -n "$LUMA_FORGE_EXECUTION_SCHEMA_PATH"

RUN python /runtime-build/extract_runtime_metadata.py \
    --execution-contract /runtime-build/catalog-execution-contract.json \
    --execution-schema /runtime-build/execution-schema.json \
    --execution-contract-output /runtime-build/execution-contract.json
```

Keep the existing Python/ComfyUI/PyTorch arguments and runtime installation
steps. Remove `LUMA_FORGE_BUNDLED_WORKFLOW_PATH`, `LUMA_FORGE_WORKFLOW_ID`,
`LUMA_FORGE_WORKFLOW_VERSION`, `workflow-catalog.json`, and
`execution-schemas.json`.

- [ ] **Step 7: Point workflow-binding smoke at the revision document**

In `workers/runpod-endpoint/tests/test_workflow_bindings.py`, set:

```python
WORKFLOW_PATH = (
    Path(__file__).resolve().parents[3]
    / "bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/workflow"
)
```

- [ ] **Step 8: Run the endpoint worker suite**

Run:

```bash
PYTHONPATH=workers/runpod-endpoint/src python3 -m unittest discover \
  -s workers/runpod-endpoint/tests
```

Expected: all tests PASS.

- [ ] **Step 9: Commit direct endpoint build inputs**

```bash
git add workers/runpod-endpoint
git commit -m "refactor(runpod): build from catalog revisions"
```

---

### Task 3: Promote endpoint images as immutable catalog revisions

**Files:**
- Modify: `workers/runpod-endpoint/release_tool.py`
- Modify: `workers/runpod-endpoint/tests/test_release_tool.py`
- Modify: `.github/workflows/deploy-runpod-endpoint.yml`
- Modify: `workers/runpod-endpoint/README.md`

**Interfaces:**
- Consumes: Task 2 `entry_file`, `next_revision`, direct resolve outputs, a digest-pinned image ref, and the selected workflow revision.
- Produces: `promote_endpoint_image(catalog_root: Path, workflow_id: str, workflow_revision: str, contract_revision: str, image_ref: str) -> tuple[Path, Path]`.
- Produces: promotion GitHub outputs `runtime_contract_path`, `workflow_revision_path`, and `promoted_workflow_revision`.

- [ ] **Step 1: Write focused immutable-promotion tests**

Use a small temporary revision tree containing only the selected workflow and
its endpoint contract. The core test must assert both creation and immutability:

```python
def test_promote_creates_contract_and_workflow_revisions_without_mutating_source(self):
    with tempfile.TemporaryDirectory() as directory:
        catalog_root = _write_catalog_tree(Path(directory))
        source_requirements = (
            catalog_root / "entries/workflows/workflow/1.0.0/contract_requirements"
        ).read_text(encoding="utf-8")

        contract_path, workflow_path = release_tool.promote_endpoint_image(
            catalog_root=catalog_root,
            workflow_id="workflow",
            workflow_revision="1.0.0",
            contract_revision="1.0.1",
            image_ref=_image_ref("4"),
        )

        self.assertEqual({"image_ref": _image_ref("4")}, json.loads(contract_path.read_text()))
        promoted = json.loads((workflow_path / "contract_requirements").read_text())
        self.assertEqual(
            "1.0.1",
            promoted["contract_requirements"][0]["endpoint_contract_ref"]["revision"],
        )
        self.assertEqual(
            source_requirements,
            (catalog_root / "entries/workflows/workflow/1.0.0/contract_requirements").read_text(),
        )
```

Keep one rejection test for a mutable image ref and one for a mismatched
endpoint contract ID. Delete old in-memory aggregate mutation tests.

Define the temporary tree locally in the same test file; do not bind promotion
tests to the real packaged workflow:

```python
WORKFLOW_DOCUMENTS = {
    "metadata": {"name": "Workflow"},
    "model_assets": {"model_assets": []},
    "contract_requirements": {
        "contract_requirements": [{
            "runtime_type": "runpod",
            "endpoint_contract_ref": {
                "contract": "catalog/contracts/runtime_contract_revision",
                "id": "runpod-endpoint-workflow",
                "revision": "1.0.0",
            },
            "provisioner_contract_ref": {
                "contract": "catalog/contracts/runtime_contract_revision",
                "id": "provisioner",
                "revision": "1.0.0",
            },
        }],
    },
    "execution_contract": {"schema_ref": {}, "input_bindings": []},
    "workflow": {"nodes": [], "links": []},
}


def _write_catalog_tree(root: Path) -> Path:
    catalog_root = root / "catalog"
    workflow = catalog_root / "entries/workflows/workflow/1.0.0"
    workflow.mkdir(parents=True)
    for name, value in WORKFLOW_DOCUMENTS.items():
        (workflow / name).write_text(json.dumps(value), encoding="utf-8")
    contract = (
        catalog_root
        / "entries/runtime_contracts/runpod-endpoint-workflow/1.0.0/runtime_contract"
    )
    contract.parent.mkdir(parents=True)
    contract.write_text(json.dumps({"image_ref": _image_ref("2")}), encoding="utf-8")
    return catalog_root
```

- [ ] **Step 2: Run the focused tests and verify they fail**

Run:

```bash
PYTHONPATH=workers/runpod-endpoint/src python3 -m unittest discover \
  -s workers/runpod-endpoint/tests -p 'test_release_tool.py'
```

Expected: FAIL because promotion still mutates aggregate JSON objects.

- [ ] **Step 3: Implement immutable endpoint promotion**

Add `shutil` and implement:

```python
WORKFLOW_FILES = (
    "metadata",
    "model_assets",
    "contract_requirements",
    "execution_contract",
    "workflow",
)


def promote_endpoint_image(
    *,
    catalog_root: Path,
    workflow_id: str,
    workflow_revision: str,
    contract_revision: str,
    image_ref: str,
) -> tuple[Path, Path]:
    _validate_image_ref(image_ref)
    contract_id = endpoint_contract_id(workflow_id)
    _parse_semver(contract_revision)
    source = entry_file(
        catalog_root, "workflow", workflow_id, workflow_revision, "workflow"
    ).parent
    requirements_path = source / "contract_requirements"
    requirements = _load_json(requirements_path)
    runpod = _runpod_contract_requirements(requirements)
    endpoint_ref = _dict_value(runpod, "endpoint_contract_ref")
    if endpoint_ref.get("id") != contract_id:
        raise ReleaseToolError(
            f"workflow revision does not reference endpoint contract: {contract_id}"
        )

    contract_dir = (
        catalog_root / "entries/runtime_contracts" / contract_id / contract_revision
    )
    workflow_root = catalog_root / "entries/workflows" / workflow_id
    promoted_revision = next_revision(workflow_root)
    promoted_dir = workflow_root / promoted_revision
    if contract_dir.exists() or promoted_dir.exists():
        raise ReleaseToolError("catalog promotion revision already exists")
    for name in WORKFLOW_FILES:
        if not (source / name).is_file():
            raise ReleaseToolError(f"workflow revision file does not exist: {source / name}")

    endpoint_ref["revision"] = contract_revision
    contract_dir.mkdir(parents=True)
    _write_json(contract_dir / "runtime_contract", {"image_ref": image_ref})
    shutil.copytree(source, promoted_dir)
    _write_json(promoted_dir / "contract_requirements", requirements)
    return contract_dir / "runtime_contract", promoted_dir
```

Change `_runpod_contract_requirements` to read the direct document's
`contract_requirements` array and its `endpoint_contract_ref`/
`provisioner_contract_ref` keys.

Change `promote-endpoint-image` CLI to accept `--catalog-root`,
`--workflow-id`, `--workflow-revision`, `--contract-revision`, `--image-ref`,
and optional `--github-output`. Remove `--runtime-preset`, `--catalog`, and
`--workflow-catalog`. Write repository-relative paths when the catalog root is
relative; reject newline-bearing GitHub outputs with the existing writer.

- [ ] **Step 4: Update the endpoint GitHub workflow**

Resolve with:

```yaml
python workers/runpod-endpoint/release_tool.py resolve \
  --catalog-root bundled/catalog \
  --workflow-id "$WORKFLOW_ID" \
  --workflow-revision "$WORKFLOW_REVISION" \
  --github-output "$GITHUB_OUTPUT"
```

Pass these Docker arguments:

```yaml
--build-arg LUMA_FORGE_WORKFLOW_PATH="${{ steps.contract.outputs.workflow_path }}" \
--build-arg LUMA_FORGE_EXECUTION_CONTRACT_PATH="${{ steps.contract.outputs.execution_contract_path }}" \
--build-arg LUMA_FORGE_EXECUTION_SCHEMA_PATH="${{ steps.contract.outputs.execution_schema_path }}"
```

Give promotion `id: promotion` and run:

```yaml
python workers/runpod-endpoint/release_tool.py promote-endpoint-image \
  --catalog-root bundled/catalog \
  --workflow-id "${{ steps.contract.outputs.workflow_id }}" \
  --workflow-revision "${{ steps.contract.outputs.workflow_revision }}" \
  --contract-revision "${{ steps.contract.outputs.contract_revision }}" \
  --image-ref "${{ steps.digest.outputs.endpoint_ref }}" \
  --github-output "$GITHUB_OUTPUT"
```

Rename workflow input/output vocabulary from `workflow_version`/
`contract_version` to `workflow_revision`/`contract_revision`.

Scope-check only the exact runtime contract file and the five files under the
exact promoted workflow directory. Stage only:

```yaml
add-paths: |
  ${{ steps.promotion.outputs.runtime_contract_path }}
  ${{ steps.promotion.outputs.workflow_revision_path }}
```

- [ ] **Step 5: Update endpoint operational documentation**

In `workers/runpod-endpoint/README.md`, replace old flat paths and in-place
Workflow Preset wording with the direct revision inputs and immutable promotion
behavior. Keep runtime/container behavior unchanged.

- [ ] **Step 6: Run endpoint release verification**

Run:

```bash
PYTHONPATH=workers/runpod-endpoint/src python3 -m unittest discover \
  -s workers/runpod-endpoint/tests
rg -n 'workflow-catalog\.json|execution-schemas\.json|runtime-contracts\.json|bundled/runtime-presets|bundled/workflows' \
  workers/runpod-endpoint .github/workflows/deploy-runpod-endpoint.yml
```

Expected: tests PASS and `rg` returns no matches.

- [ ] **Step 7: Commit endpoint promotion**

```bash
git add workers/runpod-endpoint .github/workflows/deploy-runpod-endpoint.yml
git commit -m "refactor(runpod): promote immutable catalog revisions"
```

---

### Task 4: Promote provisioner images as immutable catalog revisions

**Files:**
- Modify: `workers/provisioner/release_tool.py`
- Modify: `workers/provisioner/tests/test_release_tool.py`
- Modify: `.github/workflows/deploy-provisioner.yml`
- Modify: `workers/provisioner/README.md`

**Interfaces:**
- Consumes: `bundled/catalog`, the fixed contract ID `provisioner`, and a digest-pinned image ref.
- Produces: `next_provisioner_contract_revision(catalog_root: Path, contract_id: str) -> str`.
- Produces: `promote_provisioner_image(catalog_root: Path, contract_id: str, contract_revision: str, image_ref: str) -> tuple[Path, list[Path]]`.

- [ ] **Step 1: Replace aggregate tests with filesystem revision tests**

Use a temporary catalog containing one `provisioner/1.0.0/runtime_contract` and
one workflow revision. Add these focused assertions:

```python
def test_next_provisioner_contract_revision_uses_directory_names(self):
    with tempfile.TemporaryDirectory() as directory:
        catalog_root = _write_catalog_tree(Path(directory))
        self.assertEqual(
            "1.0.1",
            release_tool.next_provisioner_contract_revision(catalog_root, "provisioner"),
        )


def test_promote_creates_new_contract_and_latest_workflow_revisions(self):
    with tempfile.TemporaryDirectory() as directory:
        catalog_root = _write_catalog_tree(Path(directory))
        contract_path, workflows = release_tool.promote_provisioner_image(
            catalog_root=catalog_root,
            contract_id="provisioner",
            contract_revision="1.0.1",
            image_ref=_image_ref("4"),
        )
        self.assertEqual({"image_ref": _image_ref("4")}, json.loads(contract_path.read_text()))
        self.assertEqual([catalog_root / "entries/workflows/workflow/1.0.1"], workflows)
        promoted = json.loads((workflows[0] / "contract_requirements").read_text())
        self.assertEqual(
            "1.0.1",
            promoted["contract_requirements"][0]["provisioner_contract_ref"]["revision"],
        )
        self.assertEqual(
            "1.0.0",
            json.loads(
                (catalog_root / "entries/workflows/workflow/1.0.0/contract_requirements").read_text()
            )["contract_requirements"][0]["provisioner_contract_ref"]["revision"],
        )
```

Keep one mutable-image rejection test and the GitHub workflow order/scope test.
Delete aggregate `contracts` and `workflow_presets` fixtures.

Define this complete local tree helper in the provisioner test file:

```python
def _write_catalog_tree(root: Path) -> Path:
    catalog_root = root / "catalog"
    contract = catalog_root / "entries/runtime_contracts/provisioner/1.0.0/runtime_contract"
    contract.parent.mkdir(parents=True)
    contract.write_text(json.dumps({"image_ref": _image_ref("2")}), encoding="utf-8")

    workflow = catalog_root / "entries/workflows/workflow/1.0.0"
    workflow.mkdir(parents=True)
    documents = {
        "metadata": {"name": "Workflow"},
        "model_assets": {"model_assets": []},
        "contract_requirements": {
            "contract_requirements": [{
                "runtime_type": "runpod",
                "endpoint_contract_ref": {
                    "contract": "catalog/contracts/runtime_contract_revision",
                    "id": "runpod-endpoint-workflow",
                    "revision": "1.0.0",
                },
                "provisioner_contract_ref": {
                    "contract": "catalog/contracts/runtime_contract_revision",
                    "id": "provisioner",
                    "revision": "1.0.0",
                },
            }],
        },
        "execution_contract": {"schema_ref": {}, "input_bindings": []},
        "workflow": {"nodes": [], "links": []},
    }
    for name, value in documents.items():
        (workflow / name).write_text(json.dumps(value), encoding="utf-8")
    return catalog_root
```

- [ ] **Step 2: Run provisioner tests and verify they fail**

Run:

```bash
PYTHONPATH=workers/provisioner/src python3 -m unittest discover \
  -s workers/provisioner/tests -p 'test_release_tool.py'
```

Expected: FAIL because the tool still accepts aggregate JSON catalogs.

- [ ] **Step 3: Implement direct provisioner revision handling**

Use only `json`, `re`, `shutil`, and `pathlib`. Implement the same local safe-ID,
semver, JSON read/write, next-revision, and workflow file checks as the endpoint
tool; do not introduce a cross-worker package for these few stable helpers.

Core behavior:

```python
def _latest_revision(entries_root: Path) -> tuple[str, Path]:
    if not entries_root.is_dir():
        raise ReleaseToolError(f"catalog entry has no revisions: {entries_root}")
    candidates = [
        (_parse_semver(path.name), path)
        for path in entries_root.iterdir()
        if path.is_dir()
    ]
    if not candidates:
        raise ReleaseToolError(f"catalog entry has no revisions: {entries_root}")
    _version, path = max(candidates, key=lambda item: item[0])
    return path.name, path


def next_provisioner_contract_revision(catalog_root: Path, contract_id: str) -> str:
    if not _is_safe_identifier(contract_id):
        raise ReleaseToolError("invalid runtime contract id")
    revision, _path = _latest_revision(
        catalog_root / "entries/runtime_contracts" / contract_id
    )
    major, minor, patch = _parse_semver(revision)
    return _format_semver((major, minor, patch + 1))


def _next_revision(entries_root: Path) -> str:
    revision, _path = _latest_revision(entries_root)
    major, minor, patch = _parse_semver(revision)
    return _format_semver((major, minor, patch + 1))


def promote_provisioner_image(
    *,
    catalog_root: Path,
    contract_id: str,
    contract_revision: str,
    image_ref: str,
) -> tuple[Path, list[Path]]:
    _validate_image_ref(image_ref)
    if not _is_safe_identifier(contract_id):
        raise ReleaseToolError("invalid runtime contract id")
    _parse_semver(contract_revision)
    contract_dir = (
        catalog_root / "entries/runtime_contracts" / contract_id / contract_revision
    )
    if contract_dir.exists():
        raise ReleaseToolError(f"runtime contract revision already exists: {contract_revision}")

    promotions: list[tuple[Path, Path, dict[str, Any]]] = []
    workflows_root = catalog_root / "entries/workflows"
    for workflow_root in sorted(path for path in workflows_root.iterdir() if path.is_dir()):
        _source_revision, source = _latest_revision(workflow_root)
        requirements = _load_json(source / "contract_requirements")
        runpod = _runpod_contract_requirements(requirements)
        reference = _dict_value(runpod, "provisioner_contract_ref")
        if reference.get("id") != contract_id:
            continue
        destination = workflow_root / _next_revision(workflow_root)
        if destination.exists():
            raise ReleaseToolError(f"workflow revision already exists: {destination}")
        reference["revision"] = contract_revision
        promotions.append((source, destination, requirements))

    if not promotions:
        raise ReleaseToolError(f"workflow catalog does not reference provisioner contract: {contract_id}")

    contract_dir.mkdir(parents=True)
    _write_json(contract_dir / "runtime_contract", {"image_ref": image_ref})
    destinations = []
    for source, destination, requirements in promotions:
        shutil.copytree(source, destination)
        _write_json(destination / "contract_requirements", requirements)
        destinations.append(destination)
    return contract_dir / "runtime_contract", destinations
```

Validate every source workflow file and every destination before the first
write. Change both CLI commands to use `--catalog-root`; use
`--contract-revision` vocabulary. `resolve-provisioner` writes `contract_id`
and `contract_revision`. Promotion writes `runtime_contract_path` to GitHub
outputs.

- [ ] **Step 4: Update the provisioner GitHub workflow**

Use `--catalog-root bundled/catalog` for resolve and promotion. Replace all
`contract_version` output references with `contract_revision`.

The scope check must require the exact provisioner runtime contract file, require
at least one new workflow revision file, and reject any path outside:

```text
bundled/catalog/entries/runtime_contracts/provisioner/<revision>/runtime_contract
bundled/catalog/entries/workflows/<safe-id>/<semver>/{metadata,model_assets,contract_requirements,execution_contract,workflow}
```

The PR action stages:

```yaml
add-paths: |
  ${{ steps.promotion.outputs.runtime_contract_path }}
  bundled/catalog/entries/workflows
```

Keep the clean-checkout assumption; the scope check prevents unrelated workflow
files from entering the PR.

- [ ] **Step 5: Update provisioner documentation**

In `workers/provisioner/README.md`, describe new runtime contract and workflow
revision directories. Remove references to `runtime-contracts.json`,
`workflow-catalog.json`, and in-place Workflow Preset updates.

- [ ] **Step 6: Run both worker suites**

Run:

```bash
PYTHONPATH=workers/provisioner/src python3 -m unittest discover \
  -s workers/provisioner/tests
PYTHONPATH=workers/runpod-endpoint/src python3 -m unittest discover \
  -s workers/runpod-endpoint/tests
```

Expected: all tests PASS.

- [ ] **Step 7: Audit live source paths**

Run:

```bash
rg -n 'workflow-catalog\.json|execution-schemas\.json|runtime-contracts\.json|bundled/runtime-presets|bundled/workflows' \
  src-tauri src workers .github \
  docs/2026-07-11-application-ports-adapters-design.md
rg -n 'new_bundled' src-tauri src workers .github \
  docs/2026-07-11-application-ports-adapters-design.md
```

Expected: both searches return no matches. Historical plans/specs and the
cutover design document are intentionally outside this live-source audit.

- [ ] **Step 8: Commit provisioner promotion**

```bash
git add workers/provisioner .github/workflows/deploy-provisioner.yml
git commit -m "refactor(provisioner): promote immutable catalog revisions"
```

---

### Task 5: Final repository verification

**Files:**
- Modify only files required by failures caused by Tasks 1-4.

**Interfaces:**
- Consumes: all prior task commits.
- Produces: verified native, worker, generated-contract, and frontend build state with a clean worktree.

- [ ] **Step 1: Run native verification**

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: all commands PASS.

- [ ] **Step 2: Run worker verification**

```bash
PYTHONPATH=workers/provisioner/src python3 -m unittest discover \
  -s workers/provisioner/tests
PYTHONPATH=workers/runpod-endpoint/src python3 -m unittest discover \
  -s workers/runpod-endpoint/tests
```

Expected: all tests PASS.

- [ ] **Step 3: Verify generated contracts and frontend**

The Tauri command interface should be unchanged, but the active branch already
contains generated facade work. Re-run its required checks:

```bash
bun run codegen:commands
bun run build
```

Expected: both commands PASS and code generation produces no diff.

- [ ] **Step 4: Run final path and secret audits**

```bash
test ! -e new_bundled
test -d bundled/catalog/contracts
test -d bundled/catalog/entries
test -d bundled/catalog/schemas
rg -n 'workflow-catalog\.json|execution-schemas\.json|runtime-contracts\.json|bundled/runtime-presets|bundled/workflows|new_bundled' \
  src-tauri src workers .github \
  docs/2026-07-11-application-ports-adapters-design.md
rg -n 'api[_-]?key|bearer|token|secret' bundled/catalog/entries
```

Expected: the path audit has no matches. Review any secret-audit match in
context; only UI-safe boolean requirements such as
`requires_hugging_face_api_key` are allowed, never credential values.

- [ ] **Step 5: Inspect the final diff and worktree**

```bash
git diff --check
git status --short
git diff --stat origin/codex/rust-side-persistence-iteration-1...HEAD
```

Expected: no whitespace errors, no generated drift, no uncommitted changes, and
the diff contains only the approved cutover plus its design/plan documents.

- [ ] **Step 6: Commit only if verification required a scoped fix**

If a failure exposed a cutover regression, add only the directly related files
and commit:

```bash
git commit -m "fix(bundled): complete catalog cutover"
```

If all verification passes without edits, do not create an empty commit.
