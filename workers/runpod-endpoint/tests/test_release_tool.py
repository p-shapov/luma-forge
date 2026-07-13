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

spec = importlib.util.spec_from_file_location("runpod_endpoint_release_tool", TOOL_PATH)
release_tool = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(release_tool)


class RunpodEndpointReleaseToolTests(unittest.TestCase):
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
