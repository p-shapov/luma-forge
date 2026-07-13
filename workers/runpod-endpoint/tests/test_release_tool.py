import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
TOOL_PATH = ROOT / "workers/runpod-endpoint/release_tool.py"
CATALOG_ROOT = ROOT / "bundled/catalog"
WORKFLOW_ID = "comfyui-hidream-o1-dev"
WORKFLOW_REVISION = "1.0.0"
ENDPOINT_DOCKERFILE_PATH = ROOT / "workers/runpod-endpoint/Dockerfile"

WORKFLOW_DOCUMENTS = {
    "metadata": {"name": "Workflow"},
    "model_assets": {"model_assets": []},
    "contract_requirements": {
        "contract_requirements": [
            {
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
            }
        ],
    },
    "execution_contract": {"schema_ref": {}, "input_bindings": []},
    "workflow": {"nodes": [], "links": []},
}


def _image_ref(digit: str) -> str:
    return f"ghcr.io/example/worker@sha256:{digit * 64}"


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

spec = importlib.util.spec_from_file_location("runpod_endpoint_release_tool", TOOL_PATH)
release_tool = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(release_tool)


class RunpodEndpointReleaseToolTests(unittest.TestCase):
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

            self.assertEqual(
                {"image_ref": _image_ref("4")},
                json.loads(contract_path.read_text(encoding="utf-8")),
            )
            promoted = json.loads(
                (workflow_path / "contract_requirements").read_text(encoding="utf-8")
            )
            self.assertEqual(
                "1.0.1",
                promoted["contract_requirements"][0]["endpoint_contract_ref"]["revision"],
            )
            self.assertEqual(
                source_requirements,
                (
                    catalog_root / "entries/workflows/workflow/1.0.0/contract_requirements"
                ).read_text(encoding="utf-8"),
            )

    def test_promote_rejects_mutable_image_ref(self):
        with tempfile.TemporaryDirectory() as directory:
            catalog_root = _write_catalog_tree(Path(directory))

            with self.assertRaisesRegex(release_tool.ReleaseToolError, "digest-pinned"):
                release_tool.promote_endpoint_image(
                    catalog_root=catalog_root,
                    workflow_id="workflow",
                    workflow_revision="1.0.0",
                    contract_revision="1.0.1",
                    image_ref="ghcr.io/example/worker:latest",
                )

    def test_promote_rejects_mismatched_endpoint_contract_id(self):
        with tempfile.TemporaryDirectory() as directory:
            catalog_root = _write_catalog_tree(Path(directory))
            requirements_path = (
                catalog_root / "entries/workflows/workflow/1.0.0/contract_requirements"
            )
            requirements = json.loads(requirements_path.read_text(encoding="utf-8"))
            requirements["contract_requirements"][0]["endpoint_contract_ref"]["id"] = "other"
            requirements_path.write_text(json.dumps(requirements), encoding="utf-8")

            with self.assertRaisesRegex(
                release_tool.ReleaseToolError,
                "workflow revision does not reference endpoint contract",
            ):
                release_tool.promote_endpoint_image(
                    catalog_root=catalog_root,
                    workflow_id="workflow",
                    workflow_revision="1.0.0",
                    contract_revision="1.0.1",
                    image_ref=_image_ref("4"),
                )

    def test_promote_rejects_symlinked_workflow_revision_before_writing(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            catalog_root = _write_catalog_tree(root)
            source = catalog_root / "entries/workflows/workflow/1.0.0"
            symlink_target = root / "workflow-source"
            source.rename(symlink_target)
            source.symlink_to(symlink_target, target_is_directory=True)
            contract_dir = catalog_root / (
                "entries/runtime_contracts/runpod-endpoint-workflow/1.0.1"
            )

            with self.assertRaisesRegex(
                release_tool.ReleaseToolError, "must not be a symlink"
            ):
                release_tool.promote_endpoint_image(
                    catalog_root=catalog_root,
                    workflow_id="workflow",
                    workflow_revision="1.0.0",
                    contract_revision="1.0.1",
                    image_ref=_image_ref("4"),
                )

            self.assertFalse(contract_dir.exists())

    def test_promote_rejects_dangling_workflow_destination_before_writing(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            catalog_root = _write_catalog_tree(root)
            destination = catalog_root / "entries/workflows/workflow/1.0.1"
            destination.symlink_to(root / "missing", target_is_directory=True)
            contract_dir = catalog_root / (
                "entries/runtime_contracts/runpod-endpoint-workflow/1.0.1"
            )

            with self.assertRaisesRegex(
                release_tool.ReleaseToolError, "must not be a symlink"
            ):
                release_tool.promote_endpoint_image(
                    catalog_root=catalog_root,
                    workflow_id="workflow",
                    workflow_revision="1.0.0",
                    contract_revision="1.0.1",
                    image_ref=_image_ref("4"),
                )

            self.assertFalse(contract_dir.exists())

    def test_promote_rejects_symlinked_contract_parent_before_writing(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            catalog_root = _write_catalog_tree(root)
            contract_root = catalog_root / (
                "entries/runtime_contracts/runpod-endpoint-workflow"
            )
            outside_contract_root = root / "outside-runtime-contract"
            contract_root.rename(outside_contract_root)
            contract_root.symlink_to(
                outside_contract_root, target_is_directory=True
            )
            contract_destination = outside_contract_root / "1.0.1"
            workflow_destination = (
                catalog_root / "entries/workflows/workflow/1.0.1"
            )

            with self.assertRaisesRegex(
                release_tool.ReleaseToolError, "must not be a symlink"
            ):
                release_tool.promote_endpoint_image(
                    catalog_root=catalog_root,
                    workflow_id="workflow",
                    workflow_revision="1.0.0",
                    contract_revision="1.0.1",
                    image_ref=_image_ref("4"),
                )

            self.assertFalse(contract_destination.exists())
            self.assertFalse(workflow_destination.exists())

    def test_promote_cli_writes_relative_revision_outputs(self):
        with tempfile.TemporaryDirectory(dir=ROOT) as directory:
            catalog_root = _write_catalog_tree(Path(directory)).relative_to(ROOT)
            output_path = Path(directory) / "github-output"

            exit_code = release_tool.main(
                [
                    "promote-endpoint-image",
                    "--catalog-root",
                    str(catalog_root),
                    "--workflow-id",
                    "workflow",
                    "--workflow-revision",
                    "1.0.0",
                    "--contract-revision",
                    "1.0.1",
                    "--image-ref",
                    _image_ref("4"),
                    "--github-output",
                    str(output_path),
                ]
            )

            self.assertEqual(0, exit_code)
            outputs = dict(
                line.split("=", 1)
                for line in output_path.read_text(encoding="utf-8").splitlines()
            )
            self.assertEqual(
                str(
                    catalog_root
                    / "entries/runtime_contracts/runpod-endpoint-workflow/1.0.1/runtime_contract"
                ),
                outputs["runtime_contract_path"],
            )
            self.assertEqual(
                str(catalog_root / "entries/workflows/workflow/1.0.1"),
                outputs["workflow_revision_path"],
            )
            self.assertEqual("1.0.1", outputs["promoted_workflow_revision"])

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

    def test_resolve_rejects_unsafe_workflow_id(self):
        with self.assertRaisesRegex(release_tool.ReleaseToolError, "invalid workflow id"):
            release_tool.resolve_endpoint_build(
                catalog_root=CATALOG_ROOT,
                workflow_id="../workflow",
                workflow_revision="1.0.0",
            )

    def test_endpoint_dockerfile_keeps_direct_runtime_build_inputs(self):
        dockerfile = ENDPOINT_DOCKERFILE_PATH.read_text(encoding="utf-8")
        dockerfile_lines = dockerfile.splitlines()

        self.assertIn("python -m venv --copies /opt/luma-forge/runtime/.venv", dockerfile)
        self.assertIn("ARG LUMA_FORGE_RUNTIME_PYTHON_VERSION", dockerfile_lines)
        self.assertIn("ARG LUMA_FORGE_COMFYUI_REVISION", dockerfile_lines)
        self.assertIn("ARG LUMA_FORGE_PYTORCH_INDEX_URL", dockerfile_lines)
        self.assertIn("ARG LUMA_FORGE_PYTORCH_PACKAGES_JSON", dockerfile_lines)
        self.assertIn("ARG LUMA_FORGE_WORKFLOW_PATH", dockerfile_lines)
        self.assertIn("ARG LUMA_FORGE_EXECUTION_CONTRACT_PATH", dockerfile_lines)
        self.assertIn("ARG LUMA_FORGE_EXECUTION_SCHEMA_PATH", dockerfile_lines)
        self.assertIn("comfy-cli==1.10.3", dockerfile)
        self.assertIn("test -f /opt/luma-forge/runtime/ComfyUI/main.py", dockerfile)
        self.assertIn("/opt/luma-forge/runtime/workflows/workflow.json", dockerfile)
        self.assertIn("workers/runpod-endpoint/src/tools/build_metadata.py", dockerfile)
        self.assertIn("/opt/luma-forge/runtime/contracts/execution-contract.json", dockerfile)

    def test_cli_writes_direct_resolve_outputs(self):
        with tempfile.TemporaryDirectory() as directory:
            output_path = Path(directory) / "github-output"

            exit_code = release_tool.main(
                [
                    "resolve",
                    "--catalog-root",
                    str(CATALOG_ROOT),
                    "--workflow-id",
                    WORKFLOW_ID,
                    "--workflow-revision",
                    WORKFLOW_REVISION,
                    "--github-output",
                    str(output_path),
                ]
            )

            self.assertEqual(0, exit_code)
            output = output_path.read_text(encoding="utf-8")
            self.assertIn("workflow_revision=1.0.0", output)
            self.assertIn("contract_revision=1.0.1", output)
            packages = next(line for line in output.splitlines() if line.startswith("pytorch_packages_json="))
            self.assertEqual(
                ["torch==2.9.1", "torchvision==0.24.1", "torchaudio==2.9.1"],
                json.loads(packages.removeprefix("pytorch_packages_json=")),
            )


if __name__ == "__main__":
    unittest.main()
