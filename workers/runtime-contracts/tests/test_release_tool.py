import importlib.util
import json
import shutil
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
TOOL_PATH = ROOT / "workers/runtime-contracts/release_tool.py"
CONTRACT_PATH = ROOT / "workers/runtime-contracts/comfyui-python312-cu121.yaml"
SCHEMA_PATH = ROOT / "workers/runtime-contracts/schema.json"
PROVISIONER_DOCKERFILE_PATH = ROOT / "workers/provisioner/Dockerfile"
ENDPOINT_DOCKERFILE_PATH = ROOT / "workers/runpod-endpoint/Dockerfile"
CATALOG_PATH = ROOT / "bundled/runtime-catalog.json"
WORKFLOW_PATH = ROOT / ".github/workflows/deploy-runtime-contract.yml"
PROVISIONER_WORKFLOW_PATH = ROOT / ".github/workflows/deploy-provisioner-worker.yml"

spec = importlib.util.spec_from_file_location("runtime_contract_release_tool", TOOL_PATH)
release_tool = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(release_tool)


class ReleaseToolTests(unittest.TestCase):
    def test_contract_outputs_include_worker_build_arguments(self):
        contract = release_tool.load_contract(CONTRACT_PATH)

        outputs = release_tool.contract_outputs(contract, CONTRACT_PATH)

        self.assertEqual(str(CONTRACT_PATH), outputs["contract"])
        self.assertEqual("comfyui-python312-cu121", outputs["contract_id"])
        self.assertEqual("1.0.0", outputs["contract_version"])
        self.assertEqual("3.12", outputs["runtime_python_version"])
        self.assertEqual("https://download.pytorch.org/whl/cu121", outputs["pytorch_index_url"])
        self.assertEqual(
            ["torch==2.5.1", "torchvision==0.20.1", "torchaudio==2.5.1"],
            json.loads(outputs["pytorch_packages_json"]),
        )
        self.assertEqual("aa9d2fc713664e9ffe37763f4c9240c0c3eda667", outputs["comfyui_revision"])

    def test_endpoint_dockerfile_keeps_contract_build_inputs_without_runtime_identity_metadata(self):
        dockerfile = ENDPOINT_DOCKERFILE_PATH.read_text(encoding="utf-8")

        self.assertIn("python -m venv --copies /opt/luma-forge/runtime/.venv", dockerfile)
        self.assertNotIn("LUMA_FORGE_PROVISIONER_IMAGE_REF", dockerfile)
        self.assertNotIn("runtime-contract.json", dockerfile)

    def test_endpoint_dockerfile_installs_pinned_comfy_cli_for_runtime_builder(self):
        dockerfile = ENDPOINT_DOCKERFILE_PATH.read_text(encoding="utf-8")

        self.assertIn("comfy-cli==1.7.3", dockerfile)
        self.assertNotIn("pip install --no-cache-dir comfy-cli\n", dockerfile)
        self.assertNotIn("pip install --no-cache-dir comfy-cli ", dockerfile)
        self.assertNotIn("pip install --no-cache-dir --upgrade comfy-cli", dockerfile)

    def test_endpoint_dockerfile_uses_comfy_cli_for_comfyui_runtime_install(self):
        dockerfile = ENDPOINT_DOCKERFILE_PATH.read_text(encoding="utf-8")

        self.assertIn("export VIRTUAL_ENV=/opt/luma-forge/runtime/.venv", dockerfile)
        self.assertIn("export PATH=\"${VIRTUAL_ENV}/bin:${PATH}\"", dockerfile)
        self.assertIn("comfy --skip-prompt tracking disable", dockerfile)
        self.assertIn("comfy --skip-prompt --workspace /opt/luma-forge/runtime/ComfyUI install", dockerfile)
        self.assertIn("--url https://github.com/comfyanonymous/ComfyUI.git", dockerfile)
        self.assertIn("--version nightly", dockerfile)
        self.assertIn("--commit \"$LUMA_FORGE_COMFYUI_REVISION\"", dockerfile)
        self.assertIn("--nvidia", dockerfile)
        self.assertIn("--skip-manager", dockerfile)
        self.assertIn("--skip-torch-or-directml", dockerfile)
        self.assertNotIn("--skip-requirement", dockerfile)
        self.assertNotIn("git clone", dockerfile)
        self.assertNotIn("git checkout \"$LUMA_FORGE_COMFYUI_REVISION\"", dockerfile)

    def test_endpoint_dockerfile_keeps_runtime_layout_validation_checks(self):
        dockerfile = ENDPOINT_DOCKERFILE_PATH.read_text(encoding="utf-8")

        self.assertIn("test -f /opt/luma-forge/runtime/ComfyUI/main.py", dockerfile)
        self.assertIn("test -x /opt/luma-forge/runtime/.venv/bin/python", dockerfile)
        self.assertIn("test ! -e /opt/luma-forge/runtime/ComfyUI/custom_nodes/ComfyUI-Manager", dockerfile)
        self.assertIn("/opt/luma-forge/runtime/base-runtime/pip-freeze.txt", dockerfile)
        self.assertIn("/opt/luma-forge/runtime/base-runtime/install-report.json", dockerfile)

    def test_provisioner_workflow_is_separate_from_runtime_contract_release(self):
        workflow = PROVISIONER_WORKFLOW_PATH.read_text(encoding="utf-8")

        self.assertIn("--file workers/provisioner/Dockerfile", workflow)
        self.assertIn("provisioner-worker", workflow)
        self.assertNotIn("LUMA_FORGE_RUNTIME_PYTHON_VERSION", workflow)
        self.assertNotIn("bundled/runtime-catalog.json", workflow)

    def test_provisioner_dockerfile_has_no_endpoint_runtime_contract_inputs(self):
        dockerfile = PROVISIONER_DOCKERFILE_PATH.read_text(encoding="utf-8")

        self.assertIn("COPY workers/provisioner/pyproject.toml", dockerfile)
        self.assertNotIn("runtime-builder", dockerfile)
        self.assertNotIn("runpod-endpoint", dockerfile)
        self.assertNotIn("LUMA_FORGE_RUNTIME_PYTHON_VERSION", dockerfile)
        self.assertNotIn("LUMA_FORGE_COMFYUI_REVISION", dockerfile)
        self.assertNotIn("/opt/luma-forge/runtime", dockerfile)

    def test_runtime_contract_workflow_only_publishes_endpoint_image(self):
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")

        self.assertIn("--file workers/runpod-endpoint/Dockerfile", workflow)
        self.assertIn("runpod-endpoint-worker", workflow)
        self.assertNotIn("--file workers/provisioner/Dockerfile", workflow)
        self.assertNotIn("provisioner-worker", workflow)

    def test_catalog_validation_runs_before_build_or_publish(self):
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")

        resolve_contract_index = workflow.index("Resolve contract metadata")
        validate_workers_index = workflow.index("Validate workers")
        resolve_image_index = workflow.index("Resolve image tags")
        build_endpoint_index = workflow.index("Build endpoint image")
        publish_index = workflow.index("Publish endpoint image")
        catalog_update_index = workflow.index("Generate Catalog updates")
        verify_catalog_pr_scope_index = workflow.index("Verify Catalog PR scope")
        catalog_pr_index = workflow.index("Open Catalog update PR")

        self.assertLess(resolve_contract_index, resolve_image_index)
        self.assertLess(validate_workers_index, build_endpoint_index)
        self.assertLess(validate_workers_index, publish_index)
        self.assertLess(resolve_image_index, build_endpoint_index)
        self.assertLess(publish_index, catalog_update_index)
        self.assertLess(verify_catalog_pr_scope_index, catalog_pr_index)

    def test_workflow_restricts_runtime_catalog_pr_to_catalog_file(self):
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")

        catalog_pr_section = workflow.split("Open Catalog update PR", maxsplit=1)[1]

        self.assertIn("add-paths:", catalog_pr_section)
        self.assertIn("bundled/runtime-catalog.json", catalog_pr_section)
        self.assertIn("bundled/workflow-catalog.json", catalog_pr_section)

    def test_runtime_workflow_updates_exact_resolved_catalog_version_after_publish(self):
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        publish_index = workflow.index("Publish endpoint image")
        catalog_update_index = workflow.index("Generate Catalog updates")
        catalog_update_section = workflow.split("Generate Catalog updates", maxsplit=1)[1].split(
            "Verify Catalog PR scope",
            maxsplit=1,
        )[0]

        self.assertLess(publish_index, catalog_update_index)
        self.assertIn("--contract-version \"${{ steps.contract.outputs.contract_version }}\"", catalog_update_section)

    def test_workflow_fails_on_unexpected_catalog_pr_changes(self):
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        verify_section = workflow.split("Verify Catalog PR scope", maxsplit=1)[1].split(
            "Open Catalog update PR",
            maxsplit=1,
        )[0]

        self.assertIn("git status --porcelain --untracked-files=all", verify_section)
        self.assertIn("grep -Evx 'bundled/(runtime|workflow)-catalog\\.json'", verify_section)
        self.assertIn("unexpected changed paths", verify_section)
        self.assertIn("exit 1", verify_section)

    def test_provisioner_workflow_allows_catalog_pr_permissions(self):
        workflow = PROVISIONER_WORKFLOW_PATH.read_text(encoding="utf-8")

        permissions_section = workflow.split("permissions:", maxsplit=1)[1].split("concurrency:", maxsplit=1)[0]
        self.assertIn("contents: write", permissions_section)
        self.assertIn("packages: write", permissions_section)
        self.assertIn("pull-requests: write", permissions_section)

    def test_provisioner_workflow_resolves_digest_and_updates_catalog_after_publish(self):
        workflow = PROVISIONER_WORKFLOW_PATH.read_text(encoding="utf-8")
        resolve_index = workflow.index("Resolve provisioner catalog metadata")
        tag_index = workflow.index("Resolve image tag")
        publish_index = workflow.index("Publish provisioner image")
        digest_index = workflow.index("Resolve pushed image digest")
        catalog_update_index = workflow.index("Generate Provisioner Catalog updates")

        self.assertLess(resolve_index, tag_index)
        self.assertLess(publish_index, digest_index)
        self.assertLess(digest_index, catalog_update_index)
        self.assertIn("docker inspect --format='{{index .RepoDigests 0}}'", workflow)
        self.assertIn("update-provisioner-catalog", workflow)
        self.assertIn("--provisioner-ref \"${{ steps.digest.outputs.provisioner_ref }}\"", workflow)

    def test_provisioner_workflow_fails_on_unexpected_catalog_pr_changes(self):
        workflow = PROVISIONER_WORKFLOW_PATH.read_text(encoding="utf-8")
        verify_section = workflow.split("Verify Provisioner Catalog PR scope", maxsplit=1)[1].split(
            "Open Provisioner Catalog update PR",
            maxsplit=1,
        )[0]

        self.assertIn("git status --porcelain --untracked-files=all", verify_section)
        self.assertIn("grep -Evx 'bundled/(provisioner|workflow)-catalog\\.json'", verify_section)
        self.assertIn("unexpected changed paths", verify_section)
        self.assertIn("exit 1", verify_section)

    def test_provisioner_workflow_opens_reviewed_catalog_update_pr(self):
        workflow = PROVISIONER_WORKFLOW_PATH.read_text(encoding="utf-8")
        catalog_pr_section = workflow.split("Open Provisioner Catalog update PR", maxsplit=1)[1]

        self.assertIn("add-paths:", catalog_pr_section)
        self.assertIn("bundled/provisioner-catalog.json", catalog_pr_section)
        self.assertIn("bundled/workflow-catalog.json", catalog_pr_section)
        self.assertIn(
            "branch: provisioner-catalog/${{ steps.contract.outputs.contract_id }}-${{ steps.contract.outputs.contract_version }}",
            catalog_pr_section,
        )
        self.assertIn("commit-message: \"chore(workers): catalog provisioner image\"", catalog_pr_section)
        self.assertIn("Published image: `${{ steps.digest.outputs.provisioner_ref }}`", catalog_pr_section)
        self.assertIn("Catalog version: `${{ steps.contract.outputs.contract_version }}`", catalog_pr_section)

    def test_find_contract_matches_contract_id(self):
        contract = release_tool.load_contract(CONTRACT_PATH)
        catalog = _catalog_with_contract(contract, endpoint_ref=_image_ref("2"))

        contract = release_tool.find_contract(catalog, "comfyui-python312-cu121")

        self.assertIsNotNone(contract)
        assert contract is not None
        revision = release_tool.find_revision(contract, "1.0.0")
        self.assertIsNotNone(revision)
        assert revision is not None
        self.assertEqual(_image_ref("2"), revision["endpoint_image_ref"])

    def test_update_catalog_creates_new_contract_from_endpoint_image_ref(self):
        contract = release_tool.load_contract(CONTRACT_PATH)
        catalog = {"contracts": []}

        updated = release_tool.update_catalog(
            contract=contract,
            catalog=catalog,
            endpoint_ref=_image_ref("2"),
        )

        self.assertEqual(
            [
                {
                    "id": "comfyui-python312-cu121",
                    "revisions": [
                        {
                            "version": "1.0.0",
                            "endpoint_image_ref": _image_ref("2"),
                        }
                    ],
                }
            ],
            updated["contracts"],
        )

    def test_update_catalog_appends_bumped_revision_for_existing_contract(self):
        contract = release_tool.load_contract(CONTRACT_PATH)
        catalog = _catalog_with_contract(contract, endpoint_ref=_image_ref("2"))

        updated = release_tool.update_catalog(
            contract=contract,
            catalog=catalog,
            endpoint_ref=_image_ref("4"),
        )

        self.assertEqual(1, len(updated["contracts"]))
        self.assertEqual(2, len(updated["contracts"][0]["revisions"]))
        self.assertEqual("1.0.0", updated["contracts"][0]["revisions"][0]["version"])
        self.assertEqual("1.0.1", updated["contracts"][0]["revisions"][1]["version"])
        self.assertEqual(_image_ref("4"), updated["contracts"][0]["revisions"][1]["endpoint_image_ref"])

    def test_update_catalog_rejects_duplicate_explicit_revision(self):
        contract = release_tool.load_contract(CONTRACT_PATH)
        catalog = _catalog_with_contract(contract, endpoint_ref=_image_ref("2"))

        with self.assertRaisesRegex(release_tool.ReleaseToolError, "already exists"):
            release_tool.update_catalog(
                contract=contract,
                catalog=catalog,
                endpoint_ref=_image_ref("4"),
                contract_version="1.0.0",
            )

    def test_next_contract_version_uses_contract_major_bump(self):
        contract = release_tool.load_contract(CONTRACT_PATH)
        contract["contract"]["version"] = "2.0.0"
        catalog = _catalog_with_contract(contract, endpoint_ref=_image_ref("2"))
        catalog["contracts"][0]["revisions"][0]["version"] = "1.0.0"

        self.assertEqual("2.0.0", release_tool.next_contract_version(contract=contract, catalog=catalog))

    def test_update_workflow_catalog_points_presets_at_bumped_revision(self):
        workflow_catalog = {
            "workflow_presets": [
                {
                    "id": "preset",
                    "runtime_contract": {
                        "id": "comfyui-python312-cu121",
                        "version": "1.0.0",
                    },
                }
            ]
        }

        updated = release_tool.update_workflow_catalog(
            catalog=workflow_catalog,
            contract_id="comfyui-python312-cu121",
            contract_version="1.0.1",
        )

        self.assertEqual("1.0.1", updated["workflow_presets"][0]["runtime_contract"]["version"])

    def test_update_catalog_rejects_mutable_endpoint_image_refs(self):
        contract = release_tool.load_contract(CONTRACT_PATH)
        catalog = {"contracts": []}

        with self.assertRaisesRegex(release_tool.ReleaseToolError, "digest-pinned"):
            release_tool.update_catalog(
                contract=contract,
                catalog=catalog,
                endpoint_ref="ghcr.io/luma-forge/test:latest",
            )

    def test_bundled_catalog_uses_simplified_contract_entries(self):
        catalog = json.loads(CATALOG_PATH.read_text(encoding="utf-8"))

        self.assertIn("contracts", catalog)
        self.assertNotIn("runtime_contracts", catalog)
        self.assertEqual({"id", "revisions"}, set(catalog["contracts"][0]))
        self.assertEqual(
            {"version", "endpoint_image_ref"},
            set(catalog["contracts"][0]["revisions"][0]),
        )
        release_tool.validate_catalog_compatibility(
            contract=release_tool.load_contract(CONTRACT_PATH),
            catalog=catalog,
        )

    def test_workflow_does_not_validate_runtime_archive_or_image_metadata(self):
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")

        self.assertNotIn("validate-runtime-archive", workflow)
        self.assertNotIn("validate-image-metadata", workflow)
        self.assertNotIn("base-runtime.tar.zst", workflow)

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

    def test_cli_resolve_uses_next_catalog_revision_when_catalog_is_provided(self):
        contract = release_tool.load_contract(CONTRACT_PATH)
        with tempfile.TemporaryDirectory() as directory:
            catalog_path = Path(directory) / "runtime-catalog.json"
            output_path = Path(directory) / "github-output"
            catalog_path.write_text(
                json.dumps(_catalog_with_contract(contract, endpoint_ref=_image_ref("2"))),
                encoding="utf-8",
            )

            exit_code = release_tool.main(
                [
                    "resolve",
                    "--contract",
                    str(CONTRACT_PATH),
                    "--catalog",
                    str(catalog_path),
                    "--github-output",
                    str(output_path),
                ]
            )

            self.assertEqual(0, exit_code)
            self.assertIn("contract_version=1.0.1", output_path.read_text(encoding="utf-8"))

    def test_cli_resolve_uses_next_bundled_runtime_catalog_revision(self):
        with tempfile.TemporaryDirectory() as directory:
            output_path = Path(directory) / "github-output"

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
            self.assertIn("contract_version=1.0.1", output_path.read_text(encoding="utf-8"))

    def test_cli_update_catalog_writes_endpoint_image_ref(self):
        contract = release_tool.load_contract(CONTRACT_PATH)
        with tempfile.TemporaryDirectory() as directory:
            catalog_path = Path(directory) / "runtime-catalog.json"
            catalog_path.write_text(
                json.dumps({"contracts": []}),
                encoding="utf-8",
            )

            exit_code = release_tool.main(
                [
                    "update-catalog",
                    "--contract",
                    str(CONTRACT_PATH),
                    "--catalog",
                    str(catalog_path),
                    "--endpoint-ref",
                    _image_ref("2"),
                ]
            )

            self.assertEqual(0, exit_code)
            updated = json.loads(catalog_path.read_text(encoding="utf-8"))
            self.assertEqual("comfyui-python312-cu121", updated["contracts"][0]["id"])
            self.assertEqual("1.0.0", updated["contracts"][0]["revisions"][0]["version"])
            self.assertEqual(_image_ref("2"), updated["contracts"][0]["revisions"][0]["endpoint_image_ref"])

    def test_cli_update_catalog_appends_revision_and_updates_workflow_catalog(self):
        contract = release_tool.load_contract(CONTRACT_PATH)
        with tempfile.TemporaryDirectory() as directory:
            catalog_path = Path(directory) / "runtime-catalog.json"
            workflow_path = Path(directory) / "workflow-catalog.json"
            catalog_path.write_text(
                json.dumps(_catalog_with_contract(contract, endpoint_ref=_image_ref("2"))),
                encoding="utf-8",
            )
            workflow_path.write_text(
                json.dumps(
                    {
                        "workflow_presets": [
                            {
                                "id": "preset",
                                "runtime_contract": {
                                    "id": "comfyui-python312-cu121",
                                    "version": "1.0.0",
                                },
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )

            exit_code = release_tool.main(
                [
                    "update-catalog",
                    "--contract",
                    str(CONTRACT_PATH),
                    "--catalog",
                    str(catalog_path),
                    "--workflow-catalog",
                    str(workflow_path),
                    "--contract-version",
                    "1.0.1",
                    "--endpoint-ref",
                    _image_ref("4"),
                ]
            )

            self.assertEqual(0, exit_code)
            updated_catalog = json.loads(catalog_path.read_text(encoding="utf-8"))
            updated_workflow = json.loads(workflow_path.read_text(encoding="utf-8"))
            self.assertEqual("1.0.1", updated_catalog["contracts"][0]["revisions"][1]["version"])
            self.assertEqual("1.0.1", updated_workflow["workflow_presets"][0]["runtime_contract"]["version"])

    def test_next_provisioner_contract_version_uses_next_patch(self):
        catalog = _provisioner_catalog()

        version = release_tool.next_provisioner_contract_version(
            catalog=catalog,
            contract_id="luma-forge-provisioner",
        )

        self.assertEqual("1.0.1", version)

    def test_update_provisioner_catalog_appends_revision_and_preserves_metadata(self):
        catalog = _provisioner_catalog(volume_mount_path="/workspace")

        updated = release_tool.update_provisioner_catalog(
            catalog=catalog,
            contract_id="luma-forge-provisioner",
            provisioner_ref=_image_ref("4"),
        )

        revisions = updated["contracts"][0]["revisions"]
        self.assertEqual(2, len(revisions))
        self.assertEqual("1.0.0", revisions[0]["version"])
        self.assertEqual("1.0.1", revisions[1]["version"])
        self.assertEqual(_image_ref("4"), revisions[1]["provisioner_worker_image_ref"])
        self.assertEqual("/workspace", revisions[1]["volume_mount_path"])

    def test_update_provisioner_catalog_rejects_duplicate_explicit_revision(self):
        catalog = _provisioner_catalog()

        with self.assertRaisesRegex(release_tool.ReleaseToolError, "already exists"):
            release_tool.update_provisioner_catalog(
                catalog=catalog,
                contract_id="luma-forge-provisioner",
                provisioner_ref=_image_ref("4"),
                contract_version="1.0.0",
            )

    def test_update_provisioner_catalog_rejects_mutable_image_refs(self):
        catalog = _provisioner_catalog()

        with self.assertRaisesRegex(release_tool.ReleaseToolError, "digest-pinned"):
            release_tool.update_provisioner_catalog(
                catalog=catalog,
                contract_id="luma-forge-provisioner",
                provisioner_ref="ghcr.io/luma-forge/provisioner-worker:latest",
            )

    def test_update_provisioner_catalog_rejects_missing_required_metadata(self):
        catalog = _provisioner_catalog()
        del catalog["contracts"][0]["revisions"][0]["volume_mount_path"]

        with self.assertRaisesRegex(release_tool.ReleaseToolError, "volume_mount_path"):
            release_tool.update_provisioner_catalog(
                catalog=catalog,
                contract_id="luma-forge-provisioner",
                provisioner_ref=_image_ref("4"),
            )

    def test_update_provisioner_workflow_catalog_points_presets_at_bumped_revision(self):
        workflow_catalog = _workflow_catalog()

        updated = release_tool.update_provisioner_workflow_catalog(
            catalog=workflow_catalog,
            contract_id="luma-forge-provisioner",
            contract_version="1.0.1",
        )

        self.assertEqual("1.0.1", updated["workflow_presets"][0]["provisioner_contract"]["version"])

    def test_cli_resolve_provisioner_writes_next_catalog_revision(self):
        with tempfile.TemporaryDirectory() as directory:
            catalog_path = Path(directory) / "provisioner-catalog.json"
            output_path = Path(directory) / "github-output"
            catalog_path.write_text(json.dumps(_provisioner_catalog()), encoding="utf-8")

            exit_code = release_tool.main(
                [
                    "resolve-provisioner",
                    "--catalog",
                    str(catalog_path),
                    "--github-output",
                    str(output_path),
                ]
            )

            self.assertEqual(0, exit_code)
            output = output_path.read_text(encoding="utf-8")
            self.assertIn("contract_id=luma-forge-provisioner", output)
            self.assertIn("contract_version=1.0.1", output)

    def test_cli_update_provisioner_catalog_appends_revision_and_updates_workflow_catalog(self):
        with tempfile.TemporaryDirectory() as directory:
            catalog_path = Path(directory) / "provisioner-catalog.json"
            workflow_path = Path(directory) / "workflow-catalog.json"
            catalog_path.write_text(json.dumps(_provisioner_catalog()), encoding="utf-8")
            workflow_path.write_text(json.dumps(_workflow_catalog()), encoding="utf-8")

            exit_code = release_tool.main(
                [
                    "update-provisioner-catalog",
                    "--catalog",
                    str(catalog_path),
                    "--workflow-catalog",
                    str(workflow_path),
                    "--contract-version",
                    "1.0.1",
                    "--provisioner-ref",
                    _image_ref("4"),
                ]
            )

            self.assertEqual(0, exit_code)
            updated_catalog = json.loads(catalog_path.read_text(encoding="utf-8"))
            updated_workflow = json.loads(workflow_path.read_text(encoding="utf-8"))
            self.assertEqual("1.0.1", updated_catalog["contracts"][0]["revisions"][1]["version"])
            self.assertEqual("1.0.1", updated_workflow["workflow_presets"][0]["provisioner_contract"]["version"])

    def test_provisioner_catalog_rejects_malformed_catalog(self):
        with self.assertRaisesRegex(release_tool.ReleaseToolError, "contracts must be a list"):
            release_tool.next_provisioner_contract_version(
                catalog={"contracts": {}},
                contract_id="luma-forge-provisioner",
            )


def _catalog_with_contract(contract, *, endpoint_ref):
    return {
        "contracts": [
            {
                "id": contract["contract"]["id"],
                "revisions": [
                    {
                        "version": contract["contract"]["version"],
                        "endpoint_image_ref": endpoint_ref,
                    }
                ],
            }
        ],
    }


def _provisioner_catalog(*, volume_mount_path="/workspace"):
    return {
        "contracts": [
            {
                "id": "luma-forge-provisioner",
                "revisions": [
                    {
                        "version": "1.0.0",
                        "provisioner_worker_image_ref": _image_ref("2"),
                        "volume_mount_path": volume_mount_path,
                    }
                ],
            }
        ],
    }


def _workflow_catalog():
    return {
        "workflow_presets": [
            {
                "id": "preset",
                "runtime_contract": {
                    "id": "comfyui-python312-cu121",
                    "version": "1.0.0",
                },
                "provisioner_contract": {
                    "id": "luma-forge-provisioner",
                    "version": "1.0.0",
                },
            }
        ]
    }


def _image_ref(seed):
    return f"ghcr.io/luma-forge/test@sha256:{seed * 64}"


if __name__ == "__main__":
    unittest.main()
