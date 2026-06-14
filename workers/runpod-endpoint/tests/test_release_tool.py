import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
TOOL_PATH = ROOT / "workers/runpod-endpoint/release_tool.py"
RUNTIME_PRESET_PATH = ROOT / "runtime-presets/comfyui-py312-cu126-torch291.yaml"
ENDPOINT_DOCKERFILE_PATH = ROOT / "workers/runpod-endpoint/Dockerfile"
CATALOG_PATH = ROOT / "bundled/runtime-contracts.json"
WORKFLOW_PATH = ROOT / ".github/workflows/deploy-runpod-endpoint.yml"

spec = importlib.util.spec_from_file_location("runpod_endpoint_promotion_tool", TOOL_PATH)
release_tool = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(release_tool)


class RunpodEndpointPromotionToolTests(unittest.TestCase):
    def test_runtime_preset_outputs_include_worker_build_arguments(self):
        runtime_preset = release_tool.load_runtime_preset(RUNTIME_PRESET_PATH)

        outputs = release_tool.runtime_preset_outputs(
            runtime_preset=runtime_preset,
            runtime_preset_path=RUNTIME_PRESET_PATH,
            workflow_id="comfyui-hidream-o1-dev",
            workflow_version="1.0.0",
        )

        self.assertEqual(str(RUNTIME_PRESET_PATH), outputs["runtime_preset"])
        self.assertEqual("comfyui-py312-cu126-torch291", outputs["runtime_preset_id"])
        self.assertEqual("comfyui-hidream-o1-dev", outputs["workflow_id"])
        self.assertEqual("1.0.0", outputs["workflow_version"])
        self.assertEqual("runpod-endpoint-comfyui-hidream-o1-dev", outputs["contract_id"])
        self.assertEqual("1.0.0", outputs["contract_version"])
        self.assertEqual("text-to-image", outputs["execution_schema_id"])
        self.assertEqual("1.0.0", outputs["execution_schema_version"])
        self.assertEqual("3.12", outputs["runtime_python_version"])
        self.assertEqual("bundled/workflows/comfyui-hidream-o1-dev.json", outputs["bundled_workflow_path"])
        self.assertEqual("https://download.pytorch.org/whl/cu126", outputs["pytorch_index_url"])
        self.assertEqual(
            ["torch==2.9.1", "torchvision==0.24.1", "torchaudio==2.9.1"],
            json.loads(outputs["pytorch_packages_json"]),
        )
        self.assertEqual("ea62dc11c9a10dae52186fdcc3da033eb46018a1", outputs["comfyui_revision"])

    def test_runtime_preset_outputs_reject_missing_execution_contract(self):
        catalog = _workflow_catalog()
        del catalog["workflow_presets"][0]["revisions"][0]["execution_contract"]
        runtime_preset = release_tool.load_runtime_preset(RUNTIME_PRESET_PATH)

        with self.assertRaisesRegex(release_tool.ReleaseToolError, "execution_contract must be an object"):
            release_tool.runtime_preset_outputs(
                runtime_preset=runtime_preset,
                runtime_preset_path=RUNTIME_PRESET_PATH,
                workflow_id="comfyui-hidream-o1-dev",
                workflow_version="1.0.0",
                workflow_catalog=catalog,
            )

    def test_runtime_preset_schema_rejects_invalid_runtime_preset(self):
        with tempfile.TemporaryDirectory() as directory:
            runtime_preset_path = Path(directory) / "invalid.yaml"
            runtime_preset_path.write_text(
                """
runtime_preset:
  id: Invalid
  version: 1.0.0
runtime:
  python_version: "3.12"
  comfyui_revision: aa9d2fc713664e9ffe37763f4c9240c0c3eda667
  pytorch:
    index_url: https://download.pytorch.org/whl/cu126
    packages:
      - torch==2.9.1
""".strip(),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(release_tool.ReleaseToolError, "invalid runtime preset id|pattern mismatch|does not match"):
                release_tool.load_runtime_preset(runtime_preset_path)

    def test_runtime_preset_schema_rejects_endpoint_contract_id(self):
        with tempfile.TemporaryDirectory() as directory:
            runtime_preset_path = Path(directory) / "legacy.yaml"
            runtime_preset_path.write_text(
                """
contract:
  id: runpod-endpoint-comfyui-hidream-o1-dev
  version: 1.0.0
runtime:
  python_version: "3.12"
  comfyui_revision: aa9d2fc713664e9ffe37763f4c9240c0c3eda667
  pytorch:
    index_url: https://download.pytorch.org/whl/cu126
    packages:
      - torch==2.9.1
""".strip(),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(release_tool.ReleaseToolError, "missing required property|unsupported property|'runtime_preset' is a required property"):
                release_tool.load_runtime_preset(runtime_preset_path)

    def test_resolve_runtime_preset_path_uses_workflow_revision_runtime_preset(self):
        path = release_tool.resolve_runtime_preset_path(
            workflow_catalog=_workflow_catalog(),
            workflow_id="comfyui-hidream-o1-dev",
            workflow_version="1.0.0",
            runtime_presets_dir=ROOT / "runtime-presets",
        )

        self.assertEqual(RUNTIME_PRESET_PATH, path)

    def test_resolve_runtime_preset_path_rejects_missing_workflow_revision(self):
        with self.assertRaisesRegex(release_tool.ReleaseToolError, "workflow revision was not found"):
            release_tool.resolve_runtime_preset_path(
                workflow_catalog=_workflow_catalog(),
                workflow_id="comfyui-hidream-o1-dev",
                workflow_version="9.9.9",
                runtime_presets_dir=ROOT / "runtime-presets",
            )

    def test_resolve_bundled_workflow_path_uses_selected_workflow(self):
        with self.assertRaisesRegex(release_tool.ReleaseToolError, "other-workflow.json"):
            release_tool.resolve_bundled_workflow_path("other-workflow")

    def test_endpoint_dockerfile_keeps_runtime_build_inputs(self):
        dockerfile = ENDPOINT_DOCKERFILE_PATH.read_text(encoding="utf-8")
        dockerfile_lines = dockerfile.splitlines()

        self.assertIn("python -m venv --copies /opt/luma-forge/runtime/.venv", dockerfile)
        self.assertIn("ARG LUMA_FORGE_RUNTIME_PYTHON_VERSION", dockerfile_lines)
        self.assertIn("ARG LUMA_FORGE_COMFYUI_REVISION", dockerfile_lines)
        self.assertIn("ARG LUMA_FORGE_PYTORCH_INDEX_URL", dockerfile_lines)
        self.assertIn("ARG LUMA_FORGE_PYTORCH_PACKAGES_JSON", dockerfile_lines)
        self.assertIn("ARG LUMA_FORGE_BUNDLED_WORKFLOW_PATH", dockerfile_lines)
        self.assertIn("comfy-cli==1.10.3", dockerfile)
        self.assertIn("test -f /opt/luma-forge/runtime/ComfyUI/main.py", dockerfile)
        self.assertIn("/opt/luma-forge/runtime/workflows/workflow.json", dockerfile)
        self.assertIn("ARG LUMA_FORGE_WORKFLOW_ID", dockerfile_lines)
        self.assertIn("ARG LUMA_FORGE_WORKFLOW_VERSION", dockerfile_lines)
        self.assertIn('test -n "$LUMA_FORGE_WORKFLOW_VERSION"', dockerfile)
        self.assertIn("bundled/workflow-catalog.json", dockerfile)
        self.assertIn("bundled/execution-schemas.json", dockerfile)
        self.assertIn("workers/runpod-endpoint/src/tools/build_metadata.py", dockerfile)
        self.assertIn("/opt/luma-forge/runtime/contracts/execution-contract.json", dockerfile)

    def test_workflow_promotes_endpoint_image_after_publish(self):
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")

        publish_index = workflow.index("Publish endpoint image")
        promotion_index = workflow.index("Promote endpoint image to catalog")
        verify_index = workflow.index("Verify endpoint promotion PR scope")
        pr_index = workflow.index("Open endpoint promotion PR")
        promotion_section = workflow.split("Promote endpoint image to catalog", maxsplit=1)[1].split(
            "Verify endpoint promotion PR scope",
            maxsplit=1,
        )[0]

        self.assertLess(publish_index, promotion_index)
        self.assertLess(verify_index, pr_index)
        self.assertIn("workflow_id:", workflow)
        self.assertIn("workflow_version:", workflow)
        self.assertIn("LUMA_FORGE_WORKFLOW_ID", workflow)
        self.assertIn("LUMA_FORGE_WORKFLOW_VERSION", workflow)
        self.assertIn("workers/runpod-endpoint/release_tool.py resolve", workflow)
        self.assertIn("workers/runpod-endpoint/release_tool.py promote-endpoint-image", promotion_section)
        self.assertIn("--runtime-preset \"${{ steps.contract.outputs.runtime_preset }}\"", promotion_section)
        self.assertIn("--workflow-id \"${{ steps.contract.outputs.workflow_id }}\"", promotion_section)
        self.assertIn("--workflow-version \"${{ steps.contract.outputs.workflow_version }}\"", promotion_section)
        self.assertIn("--image-ref \"${{ steps.digest.outputs.endpoint_ref }}\"", promotion_section)
        self.assertIn("--contract-version \"${{ steps.contract.outputs.contract_version }}\"", promotion_section)
        self.assertIn("--workflow-catalog bundled/workflow-catalog.json", promotion_section)

    def test_workflow_restricts_endpoint_promotion_pr_to_catalog_files(self):
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        verify_section = workflow.split("Verify endpoint promotion PR scope", maxsplit=1)[1].split(
            "Open endpoint promotion PR",
            maxsplit=1,
        )[0]
        pr_section = workflow.split("Open endpoint promotion PR", maxsplit=1)[1]

        self.assertIn("git status --porcelain --untracked-files=all", verify_section)
        self.assertIn("Catalog promotion did not modify bundled/runtime-contracts.json", verify_section)
        self.assertIn("Catalog promotion did not modify bundled/workflow-catalog.json", verify_section)
        self.assertIn("grep -Evx 'bundled/(runtime-contracts|workflow-catalog)\\.json'", verify_section)
        self.assertIn("unexpected changed paths", verify_section)
        self.assertIn("add-paths:", pr_section)
        self.assertIn("bundled/runtime-contracts.json", pr_section)
        self.assertIn("bundled/workflow-catalog.json", pr_section)
        self.assertIn("promote endpoint image", pr_section)
        self.assertIn(
            'commit-message: "chore(workers): promote endpoint image ${{ steps.contract.outputs.contract_id }} ${{ steps.contract.outputs.contract_version }}"',
            pr_section,
        )

    def test_promote_endpoint_image_creates_new_contract_from_image_ref(self):
        runtime_preset = release_tool.load_runtime_preset(RUNTIME_PRESET_PATH)
        catalog = {"contracts": []}

        updated = release_tool.promote_endpoint_image(
            runtime_preset=runtime_preset,
            catalog=catalog,
            contract_id="runpod-endpoint-comfyui-hidream-o1-dev",
            image_ref=_image_ref("2"),
        )

        self.assertEqual(
            [
                {
                    "id": "runpod-endpoint-comfyui-hidream-o1-dev",
                    "revisions": [
                        {
                            "version": "1.0.0",
                            "image_ref": _image_ref("2"),
                        }
                    ],
                }
            ],
            updated["contracts"],
        )

    def test_promote_endpoint_image_appends_bumped_revision_for_existing_contract(self):
        runtime_preset = release_tool.load_runtime_preset(RUNTIME_PRESET_PATH)
        catalog = _catalog_with_contract("runpod-endpoint-comfyui-hidream-o1-dev", image_ref=_image_ref("2"))

        updated = release_tool.promote_endpoint_image(
            runtime_preset=runtime_preset,
            catalog=catalog,
            contract_id="runpod-endpoint-comfyui-hidream-o1-dev",
            image_ref=_image_ref("4"),
        )

        self.assertEqual(1, len(updated["contracts"]))
        self.assertEqual(2, len(updated["contracts"][0]["revisions"]))
        self.assertEqual("1.0.0", updated["contracts"][0]["revisions"][0]["version"])
        self.assertEqual("1.0.1", updated["contracts"][0]["revisions"][1]["version"])
        self.assertEqual(_image_ref("4"), updated["contracts"][0]["revisions"][1]["image_ref"])

    def test_promote_endpoint_image_rejects_duplicate_explicit_revision(self):
        runtime_preset = release_tool.load_runtime_preset(RUNTIME_PRESET_PATH)
        catalog = _catalog_with_contract("runpod-endpoint-comfyui-hidream-o1-dev", image_ref=_image_ref("2"))

        with self.assertRaisesRegex(release_tool.ReleaseToolError, "already exists"):
            release_tool.promote_endpoint_image(
                runtime_preset=runtime_preset,
                catalog=catalog,
                contract_id="runpod-endpoint-comfyui-hidream-o1-dev",
                image_ref=_image_ref("4"),
                contract_version="1.0.0",
            )

    def test_promote_endpoint_image_rejects_mutable_image_refs(self):
        runtime_preset = release_tool.load_runtime_preset(RUNTIME_PRESET_PATH)

        with self.assertRaisesRegex(release_tool.ReleaseToolError, "digest-pinned"):
            release_tool.promote_endpoint_image(
                runtime_preset=runtime_preset,
                catalog={"contracts": []},
                contract_id="runpod-endpoint-comfyui-hidream-o1-dev",
                image_ref="ghcr.io/luma-forge/test:latest",
            )

    def test_update_endpoint_workflow_catalog_updates_selected_revision_only(self):
        workflow_catalog = _workflow_catalog()

        updated = release_tool.update_endpoint_workflow_catalog(
            catalog=workflow_catalog,
            workflow_id="comfyui-hidream-o1-dev",
            workflow_version="1.0.0",
            contract_id="runpod-endpoint-comfyui-hidream-o1-dev",
            contract_version="1.0.1",
        )

        self.assertEqual(
            "1.0.1",
            updated["workflow_presets"][0]["revisions"][0]["contract_requirements"][0]["endpoint_contract"][
                "version"
            ],
        )
        self.assertEqual(
            "9.9.9",
            updated["workflow_presets"][0]["revisions"][1]["contract_requirements"][0]["endpoint_contract"][
                "version"
            ],
        )

    def test_update_endpoint_workflow_catalog_rejects_mismatched_endpoint_contract_id(self):
        workflow_catalog = _workflow_catalog()
        workflow_catalog["workflow_presets"][0]["revisions"][0]["contract_requirements"][0]["endpoint_contract"][
            "id"
        ] = "other-runtime"

        with self.assertRaisesRegex(release_tool.ReleaseToolError, "does not reference endpoint contract"):
            release_tool.update_endpoint_workflow_catalog(
                catalog=workflow_catalog,
                workflow_id="comfyui-hidream-o1-dev",
                workflow_version="1.0.0",
                contract_id="runpod-endpoint-comfyui-hidream-o1-dev",
                contract_version="1.0.1",
            )

    def test_next_contract_version_uses_runtime_preset_major_bump(self):
        runtime_preset = release_tool.load_runtime_preset(RUNTIME_PRESET_PATH)
        runtime_preset["runtime_preset"]["version"] = "2.0.0"
        catalog = _catalog_with_contract("runpod-endpoint-comfyui-hidream-o1-dev", image_ref=_image_ref("2"))
        catalog["contracts"][0]["revisions"][0]["version"] = "1.0.0"

        self.assertEqual(
            "2.0.0",
            release_tool.next_contract_version(
                runtime_preset=runtime_preset,
                catalog=catalog,
                contract_id="runpod-endpoint-comfyui-hidream-o1-dev",
            ),
        )

    def test_cli_promote_endpoint_image_appends_revision_and_updates_workflow_catalog(self):
        with tempfile.TemporaryDirectory() as directory:
            catalog_path = Path(directory) / "runtime-contracts.json"
            workflow_path = Path(directory) / "workflow-catalog.json"
            catalog_path.write_text(
                json.dumps(_catalog_with_contract("runpod-endpoint-comfyui-hidream-o1-dev", image_ref=_image_ref("2"))),
                encoding="utf-8",
            )
            workflow_path.write_text(json.dumps(_workflow_catalog()), encoding="utf-8")

            exit_code = release_tool.main(
                [
                    "promote-endpoint-image",
                    "--runtime-preset",
                    str(RUNTIME_PRESET_PATH),
                    "--workflow-id",
                    "comfyui-hidream-o1-dev",
                    "--workflow-version",
                    "1.0.0",
                    "--catalog",
                    str(catalog_path),
                    "--workflow-catalog",
                    str(workflow_path),
                    "--contract-version",
                    "1.0.1",
                    "--image-ref",
                    _image_ref("4"),
                ]
            )

            self.assertEqual(0, exit_code)
            updated_catalog = json.loads(catalog_path.read_text(encoding="utf-8"))
            updated_workflow = json.loads(workflow_path.read_text(encoding="utf-8"))
            self.assertEqual("1.0.1", updated_catalog["contracts"][0]["revisions"][1]["version"])
            self.assertEqual(
                "1.0.1",
                updated_workflow["workflow_presets"][0]["revisions"][0]["contract_requirements"][0][
                    "endpoint_contract"
                ]["version"],
            )

    def test_cli_writes_github_outputs(self):
        with tempfile.TemporaryDirectory() as directory:
            output_path = Path(directory) / "github-output"

            exit_code = release_tool.main(
                [
                    "resolve",
                    "--workflow-catalog",
                    str(ROOT / "bundled/workflow-catalog.json"),
                    "--workflow-id",
                    "comfyui-hidream-o1-dev",
                    "--workflow-version",
                    "1.0.0",
                    "--catalog",
                    str(CATALOG_PATH),
                    "--github-output",
                    str(output_path),
                ]
            )

            self.assertEqual(0, exit_code)
            output = output_path.read_text(encoding="utf-8")
            self.assertIn("runtime_preset_id=comfyui-py312-cu126-torch291", output)
            self.assertIn("contract_id=runpod-endpoint-comfyui-hidream-o1-dev", output)

    def test_cli_resolve_uses_next_bundled_endpoint_contract_revision(self):
        with tempfile.TemporaryDirectory() as directory:
            output_path = Path(directory) / "github-output"
            catalog = json.loads(CATALOG_PATH.read_text(encoding="utf-8"))
            contract = release_tool.find_contract(catalog, "runpod-endpoint-comfyui-hidream-o1-dev")
            self.assertIsNotNone(contract)
            assert contract is not None
            revisions = contract["revisions"]
            latest = max(tuple(int(part) for part in revision["version"].split(".")) for revision in revisions)
            expected_version = f"{latest[0]}.{latest[1]}.{latest[2] + 1}"

            exit_code = release_tool.main(
                [
                    "resolve",
                    "--workflow-catalog",
                    str(ROOT / "bundled/workflow-catalog.json"),
                    "--workflow-id",
                    "comfyui-hidream-o1-dev",
                    "--workflow-version",
                    "1.0.0",
                    "--catalog",
                    str(CATALOG_PATH),
                    "--github-output",
                    str(output_path),
                ]
            )

            self.assertEqual(0, exit_code)
            self.assertIn(f"contract_version={expected_version}", output_path.read_text(encoding="utf-8"))


def _catalog_with_contract(contract_id, *, image_ref):
    return {
        "contracts": [
            {
                "id": contract_id,
                "revisions": [
                    {
                        "version": "1.0.0",
                        "image_ref": image_ref,
                    }
                ],
            }
        ],
    }


def _workflow_catalog():
    return {
        "workflow_presets": [
            {
                "id": "comfyui-hidream-o1-dev",
                "revisions": [
                    {
                        "version": "1.0.0",
                        "runtime_preset": "comfyui-py312-cu126-torch291",
                        "execution_contract": {
                            "schema_ref": {
                                "id": "text-to-image",
                                "version": "1.0.0",
                            },
                            "input_bindings": [
                                {
                                    "value": "{{prompt}}",
                                    "node_id": "171",
                                    "path": ["widgets_values", "0"],
                                },
                                {
                                    "value": False,
                                    "node_id": "154",
                                    "path": ["widgets_values", "0"],
                                },
                                {
                                    "value": False,
                                    "node_id": "177",
                                    "path": ["widgets_values", "0"],
                                },
                            ],
                        },
                        "contract_requirements": [
                            {
                                "runtime_type": "runpod",
                                "endpoint_contract": {
                                    "id": "runpod-endpoint-comfyui-hidream-o1-dev",
                                    "version": "1.0.0",
                                },
                                "provisioner_contract": {
                                    "id": "provisioner",
                                    "version": "1.0.0",
                                },
                            }
                        ],
                    },
                    {
                        "version": "2.0.0",
                        "runtime_preset": "comfyui-py312-cu126-torch291",
                        "contract_requirements": [
                            {
                                "runtime_type": "runpod",
                                "endpoint_contract": {
                                    "id": "runpod-endpoint-comfyui-hidream-o1-dev",
                                    "version": "9.9.9",
                                },
                                "provisioner_contract": {
                                    "id": "provisioner",
                                    "version": "1.0.0",
                                },
                            }
                        ],
                    },
                ],
            }
        ]
    }


def _image_ref(seed):
    return f"ghcr.io/luma-forge/test@sha256:{seed * 64}"


if __name__ == "__main__":
    unittest.main()
