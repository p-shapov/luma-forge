import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[3]
TOOL_PATH = ROOT / "workers/promote-runtime-contract/release_tool.py"
CONTRACT_PATH = ROOT / "workers/promote-runtime-contract/comfyui-hidream-o1-dev.yaml"
ENDPOINT_DOCKERFILE_PATH = ROOT / "workers/runpod-endpoint/Dockerfile"
CATALOG_PATH = ROOT / "bundled/endpoint-contracts.json"
WORKFLOW_PATH = ROOT / ".github/workflows/deploy-endpoint-contract.yml"

spec = importlib.util.spec_from_file_location("endpoint_contract_promotion_tool", TOOL_PATH)
release_tool = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(release_tool)


class RuntimeContractPromotionToolTests(unittest.TestCase):
    def test_contract_outputs_include_worker_build_arguments(self):
        contract = release_tool.load_contract(CONTRACT_PATH)

        outputs = release_tool.contract_outputs(contract, CONTRACT_PATH)

        self.assertEqual(str(CONTRACT_PATH), outputs["contract"])
        self.assertEqual("comfyui-hidream-o1-dev", outputs["contract_id"])
        self.assertEqual("1.0.0", outputs["contract_version"])
        self.assertEqual("3.12", outputs["runtime_python_version"])
        self.assertEqual("bundled/workflows/comfyui-hidream-o1-dev.json", outputs["bundled_workflow_path"])
        self.assertEqual("https://download.pytorch.org/whl/cu126", outputs["pytorch_index_url"])
        self.assertEqual(
            ["torch==2.9.1", "torchvision==0.24.1", "torchaudio==2.9.1"],
            json.loads(outputs["pytorch_packages_json"]),
        )
        self.assertEqual("ea62dc11c9a10dae52186fdcc3da033eb46018a1", outputs["comfyui_revision"])

    def test_contract_rejects_missing_bundled_workflow_file(self):
        with tempfile.TemporaryDirectory() as directory:
            contract_path = Path(directory) / "missing-workflow.yaml"
            contract_path.write_text(
                """
contract:
  id: missing-workflow
  version: 1.0.0
runtime:
  python_version: "3.12"
  comfyui_revision: ea62dc11c9a10dae52186fdcc3da033eb46018a1
  pytorch:
    index_url: https://download.pytorch.org/whl/cu126
    packages:
      - torch==2.9.1
""".strip(),
                encoding="utf-8",
            )
            (Path(directory) / "schema.json").write_text(
                (ROOT / "workers/promote-runtime-contract/schema.json").read_text(encoding="utf-8"),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(release_tool.ReleaseToolError, "bundled workflow file does not exist"):
                release_tool.load_contract(contract_path)

    def test_contract_schema_rejects_invalid_contract(self):
        with tempfile.TemporaryDirectory() as directory:
            contract_path = Path(directory) / "invalid.yaml"
            contract_path.write_text(
                """
contract:
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
            (Path(directory) / "schema.json").write_text(
                (ROOT / "workers/promote-runtime-contract/schema.json").read_text(encoding="utf-8"),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(release_tool.ReleaseToolError, "invalid contract id|pattern mismatch|does not match"):
                release_tool.load_contract(contract_path)

    def test_contract_schema_rejects_legacy_workflow_preset_id(self):
        with tempfile.TemporaryDirectory() as directory:
            contract_path = Path(directory) / "legacy.yaml"
            contract_path.write_text(
                """
contract:
  id: comfyui-hidream-o1-dev
  version: 1.0.0
runtime:
  workflow_preset_id: comfyui-hidream-o1-dev
  python_version: "3.12"
  comfyui_revision: aa9d2fc713664e9ffe37763f4c9240c0c3eda667
  pytorch:
    index_url: https://download.pytorch.org/whl/cu126
    packages:
      - torch==2.9.1
""".strip(),
                encoding="utf-8",
            )
            (Path(directory) / "schema.json").write_text(
                (ROOT / "workers/promote-runtime-contract/schema.json").read_text(encoding="utf-8"),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(release_tool.ReleaseToolError, "unsupported property|Additional properties"):
                release_tool.load_contract(contract_path)

    def test_endpoint_dockerfile_keeps_contract_build_inputs(self):
        dockerfile = ENDPOINT_DOCKERFILE_PATH.read_text(encoding="utf-8")

        self.assertIn("python -m venv --copies /opt/luma-forge/runtime/.venv", dockerfile)
        self.assertIn(
            "ARG LUMA_FORGE_BUNDLED_WORKFLOW_PATH=bundled/workflows/comfyui-hidream-o1-dev.json",
            dockerfile,
        )

    def test_endpoint_dockerfile_default_pytorch_packages_arg_is_valid_json(self):
        dockerfile = ENDPOINT_DOCKERFILE_PATH.read_text(encoding="utf-8")
        match = re.search(r"^ARG LUMA_FORGE_PYTORCH_PACKAGES_JSON=(.+)$", dockerfile, re.MULTILINE)

        self.assertIsNotNone(match)
        raw_value = match.group(1)
        self.assertIn('\\"torch==2.9.1\\"', raw_value)
        self.assertEqual(
            ["torch==2.9.1", "torchvision==0.24.1", "torchaudio==2.9.1"],
            json.loads(raw_value.replace('\\"', '"')),
        )

    def test_endpoint_dockerfile_installs_pinned_comfy_cli_for_runtime_builder(self):
        dockerfile = ENDPOINT_DOCKERFILE_PATH.read_text(encoding="utf-8")

        self.assertIn("comfy-cli==1.10.3", dockerfile)

    def test_endpoint_dockerfile_keeps_git_available_in_runtime_image(self):
        dockerfile = ENDPOINT_DOCKERFILE_PATH.read_text(encoding="utf-8")
        worker_base_section = dockerfile.split("FROM worker-base AS runtime-builder", maxsplit=1)[0]

        self.assertIn("ca-certificates git", worker_base_section)

    def test_endpoint_dockerfile_uses_comfy_cli_for_comfyui_runtime_install(self):
        dockerfile = ENDPOINT_DOCKERFILE_PATH.read_text(encoding="utf-8")

        self.assertIn("export VIRTUAL_ENV=/opt/luma-forge/runtime/.venv", dockerfile)
        self.assertIn("export PATH=\"${VIRTUAL_ENV}/bin:${PATH}\"", dockerfile)
        self.assertIn("comfy --skip-prompt tracking disable", dockerfile)
        self.assertIn("comfy --skip-prompt --workspace /opt/luma-forge/runtime/ComfyUI install", dockerfile)
        self.assertIn("--url https://github.com/Comfy-Org/ComfyUI.git", dockerfile)
        self.assertIn("--version nightly", dockerfile)
        self.assertIn("--commit \"$LUMA_FORGE_COMFYUI_REVISION\"", dockerfile)
        self.assertIn("--nvidia", dockerfile)
        self.assertIn("--skip-manager", dockerfile)
        self.assertIn("--skip-torch-or-directml", dockerfile)

    def test_endpoint_dockerfile_persists_runtime_venv_on_final_image_path(self):
        dockerfile = ENDPOINT_DOCKERFILE_PATH.read_text(encoding="utf-8")
        final_image_section = dockerfile.rsplit("FROM worker-base", maxsplit=1)[1]

        self.assertIn("VIRTUAL_ENV=/opt/luma-forge/runtime/.venv", final_image_section)
        self.assertIn("PATH=\"${VIRTUAL_ENV}/bin:${PATH}\"", final_image_section)

    def test_endpoint_dockerfile_keeps_runtime_layout_validation_checks(self):
        dockerfile = ENDPOINT_DOCKERFILE_PATH.read_text(encoding="utf-8")

        self.assertIn("test -f /opt/luma-forge/runtime/ComfyUI/main.py", dockerfile)
        self.assertIn("test -x /opt/luma-forge/runtime/.venv/bin/python", dockerfile)
        self.assertIn("test -x /opt/luma-forge/runtime/.venv/bin/comfy", dockerfile)
        self.assertIn("test ! -e /opt/luma-forge/runtime/ComfyUI/custom_nodes/ComfyUI-Manager", dockerfile)
        self.assertIn("/opt/luma-forge/runtime/base-runtime/pip-freeze.txt", dockerfile)
        self.assertIn("/opt/luma-forge/runtime/base-runtime/install-report.json", dockerfile)
        self.assertIn("/opt/luma-forge/runtime/workflows/workflow.json", dockerfile)

    def test_workflow_promotes_runtime_image_after_publish(self):
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")

        publish_index = workflow.index("Publish endpoint image")
        promotion_index = workflow.index("Promote runtime image to catalog")
        verify_index = workflow.index("Verify Endpoint Contracts promotion PR scope")
        pr_index = workflow.index("Open Endpoint Contracts promotion PR")
        promotion_section = workflow.split("Promote runtime image to catalog", maxsplit=1)[1].split(
            "Verify Endpoint Contracts promotion PR scope",
            maxsplit=1,
        )[0]

        self.assertLess(publish_index, promotion_index)
        self.assertLess(verify_index, pr_index)
        self.assertIn("workers/promote-runtime-contract/release_tool.py resolve", workflow)
        self.assertIn("workers/promote-runtime-contract/release_tool.py promote-runtime-image", promotion_section)
        self.assertIn("--image-ref \"${{ steps.digest.outputs.endpoint_ref }}\"", promotion_section)
        self.assertIn("--contract-version \"${{ steps.contract.outputs.contract_version }}\"", promotion_section)
        self.assertIn("--workflow-catalog bundled/workflow-catalog.json", promotion_section)

    def test_workflow_restricts_runtime_catalog_promotion_pr_to_catalog_files(self):
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        verify_section = workflow.split("Verify Endpoint Contracts promotion PR scope", maxsplit=1)[1].split(
            "Open Endpoint Contracts promotion PR",
            maxsplit=1,
        )[0]
        pr_section = workflow.split("Open Endpoint Contracts promotion PR", maxsplit=1)[1]

        self.assertIn("git status --porcelain --untracked-files=all", verify_section)
        self.assertIn("Catalog promotion did not modify bundled/endpoint-contracts.json", verify_section)
        self.assertIn("Catalog promotion did not modify bundled/workflow-catalog.json", verify_section)
        self.assertIn("grep -Evx 'bundled/(endpoint-contracts|workflow-catalog)\\.json'", verify_section)
        self.assertIn("unexpected changed paths", verify_section)
        self.assertIn("add-paths:", pr_section)
        self.assertIn("bundled/endpoint-contracts.json", pr_section)
        self.assertIn("bundled/workflow-catalog.json", pr_section)
        self.assertIn("promote runtime image", pr_section)
        self.assertIn(
            'commit-message: "chore(workers): promote runtime image ${{ steps.contract.outputs.contract_id }} ${{ steps.contract.outputs.contract_version }}"',
            pr_section,
        )

    def test_find_contract_matches_contract_id(self):
        contract = release_tool.load_contract(CONTRACT_PATH)
        catalog = _catalog_with_contract(contract, image_ref=_image_ref("2"))

        contract = release_tool.find_contract(catalog, "comfyui-hidream-o1-dev")

        self.assertIsNotNone(contract)
        assert contract is not None
        revision = release_tool.find_revision(contract, "1.0.0")
        self.assertIsNotNone(revision)
        assert revision is not None
        self.assertEqual(_image_ref("2"), revision["image_ref"])

    def test_promote_runtime_image_creates_new_contract_from_image_ref(self):
        contract = release_tool.load_contract(CONTRACT_PATH)
        catalog = {"contracts": []}

        updated = release_tool.promote_runtime_image(
            contract=contract,
            catalog=catalog,
            image_ref=_image_ref("2"),
        )

        self.assertEqual(
            [
                {
                    "id": "comfyui-hidream-o1-dev",
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

    def test_promote_runtime_image_appends_bumped_revision_for_existing_contract(self):
        contract = release_tool.load_contract(CONTRACT_PATH)
        catalog = _catalog_with_contract(contract, image_ref=_image_ref("2"))

        updated = release_tool.promote_runtime_image(
            contract=contract,
            catalog=catalog,
            image_ref=_image_ref("4"),
        )

        self.assertEqual(1, len(updated["contracts"]))
        self.assertEqual(2, len(updated["contracts"][0]["revisions"]))
        self.assertEqual("1.0.0", updated["contracts"][0]["revisions"][0]["version"])
        self.assertEqual("1.0.1", updated["contracts"][0]["revisions"][1]["version"])
        self.assertEqual(_image_ref("4"), updated["contracts"][0]["revisions"][1]["image_ref"])

    def test_promote_runtime_image_rejects_duplicate_explicit_revision(self):
        contract = release_tool.load_contract(CONTRACT_PATH)
        catalog = _catalog_with_contract(contract, image_ref=_image_ref("2"))

        with self.assertRaisesRegex(release_tool.ReleaseToolError, "already exists"):
            release_tool.promote_runtime_image(
                contract=contract,
                catalog=catalog,
                image_ref=_image_ref("4"),
                contract_version="1.0.0",
            )

    def test_promote_runtime_image_rejects_mutable_image_refs(self):
        contract = release_tool.load_contract(CONTRACT_PATH)
        catalog = {"contracts": []}

        with self.assertRaisesRegex(release_tool.ReleaseToolError, "digest-pinned"):
            release_tool.promote_runtime_image(
                contract=contract,
                catalog=catalog,
                image_ref="ghcr.io/luma-forge/test:latest",
            )

    def test_update_runtime_workflow_catalog_updates_matching_preset(self):
        workflow_catalog = _workflow_catalog()

        updated = release_tool.update_runtime_workflow_catalog(
            catalog=workflow_catalog,
            contract_id="comfyui-hidream-o1-dev",
            contract_version="1.0.1",
        )

        self.assertEqual("1.0.1", updated["workflow_presets"][0]["endpoint_contract"]["version"])
        self.assertEqual("9.9.9", updated["workflow_presets"][1]["endpoint_contract"]["version"])

    def test_update_runtime_workflow_catalog_rejects_missing_matching_preset(self):
        with self.assertRaisesRegex(release_tool.ReleaseToolError, "workflow catalog does not contain preset"):
            release_tool.update_runtime_workflow_catalog(
                catalog={"workflow_presets": []},
                contract_id="comfyui-hidream-o1-dev",
                contract_version="1.0.1",
            )

    def test_update_runtime_workflow_catalog_rejects_mismatched_endpoint_contract_id(self):
        workflow_catalog = _workflow_catalog()
        workflow_catalog["workflow_presets"][0]["endpoint_contract"]["id"] = "other-runtime"

        with self.assertRaisesRegex(release_tool.ReleaseToolError, "does not reference endpoint contract"):
            release_tool.update_runtime_workflow_catalog(
                catalog=workflow_catalog,
                contract_id="comfyui-hidream-o1-dev",
                contract_version="1.0.1",
            )

    def test_next_contract_version_uses_contract_major_bump(self):
        contract = release_tool.load_contract(CONTRACT_PATH)
        contract["contract"]["version"] = "2.0.0"
        catalog = _catalog_with_contract(contract, image_ref=_image_ref("2"))
        catalog["contracts"][0]["revisions"][0]["version"] = "1.0.0"

        self.assertEqual("2.0.0", release_tool.next_contract_version(contract=contract, catalog=catalog))

    def test_cli_promote_runtime_image_appends_endpoint_contract_revision_and_updates_workflow_catalog(self):
        contract = release_tool.load_contract(CONTRACT_PATH)
        with tempfile.TemporaryDirectory() as directory:
            catalog_path = Path(directory) / "endpoint-contracts.json"
            workflow_path = Path(directory) / "workflow-catalog.json"
            catalog_path.write_text(
                json.dumps(_catalog_with_contract(contract, image_ref=_image_ref("2"))),
                encoding="utf-8",
            )
            workflow_path.write_text(json.dumps(_workflow_catalog()), encoding="utf-8")

            exit_code = release_tool.main(
                [
                    "promote-runtime-image",
                    "--contract",
                    str(CONTRACT_PATH),
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
            self.assertEqual("1.0.1", updated_workflow["workflow_presets"][0]["endpoint_contract"]["version"])

    def test_cli_writes_github_outputs(self):
        with tempfile.TemporaryDirectory() as directory:
            output_path = Path(directory) / "github-output"

            exit_code = release_tool.main(
                [
                    "resolve",
                    "--contract",
                    str(CONTRACT_PATH),
                    "--github-output",
                    str(output_path),
                ]
            )

            self.assertEqual(0, exit_code)
            output = output_path.read_text(encoding="utf-8")
            self.assertIn("pytorch_packages_json=", output)
            self.assertIn("contract_version=1.0.0", output)

    def test_cli_resolve_uses_next_bundled_endpoint_contract_revision(self):
        with tempfile.TemporaryDirectory() as directory:
            output_path = Path(directory) / "github-output"
            catalog = json.loads(CATALOG_PATH.read_text(encoding="utf-8"))
            revisions = catalog["contracts"][0]["revisions"]
            latest = max(tuple(int(part) for part in revision["version"].split(".")) for revision in revisions)
            expected_version = f"{latest[0]}.{latest[1]}.{latest[2] + 1}"

            exit_code = release_tool.main(
                [
                    "resolve",
                    "--contract",
                    str(CONTRACT_PATH),
                    "--catalog",
                    str(CATALOG_PATH),
                    "--github-output",
                    str(output_path),
                ]
            )

            self.assertEqual(0, exit_code)
            self.assertIn(f"contract_version={expected_version}", output_path.read_text(encoding="utf-8"))

def _catalog_with_contract(contract, *, image_ref):
    return {
        "contracts": [
            {
                "id": contract["contract"]["id"],
                "revisions": [
                    {
                        "version": contract["contract"]["version"],
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
                "endpoint_contract": {
                    "id": "comfyui-hidream-o1-dev",
                    "version": "1.0.0",
                },
            },
            {
                "id": "other-preset",
                "endpoint_contract": {
                    "id": "comfyui-hidream-o1-dev",
                    "version": "9.9.9",
                },
            },
        ]
    }


def _image_ref(seed):
    return f"ghcr.io/luma-forge/test@sha256:{seed * 64}"


if __name__ == "__main__":
    unittest.main()
