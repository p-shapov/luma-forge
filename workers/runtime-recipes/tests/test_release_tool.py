import importlib.util
import json
import shutil
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
TOOL_PATH = ROOT / "workers/runtime-recipes/release_tool.py"
RECIPE_PATH = ROOT / "workers/runtime-recipes/comfyui-python312-cu121.yaml"
SCHEMA_PATH = ROOT / "workers/runtime-recipes/schema.json"
DOCKERFILE_PATH = ROOT / "workers/Dockerfile"
CATALOG_PATH = ROOT / "bundled/runtime-catalog.json"
WORKFLOW_PATH = ROOT / ".github/workflows/deploy-runtime-recipe.yml"

spec = importlib.util.spec_from_file_location("runtime_recipe_release_tool", TOOL_PATH)
release_tool = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(release_tool)


class ReleaseToolTests(unittest.TestCase):
    def test_recipe_outputs_include_worker_build_arguments(self):
        recipe = release_tool.load_recipe(RECIPE_PATH)

        outputs = release_tool.recipe_outputs(recipe, RECIPE_PATH)

        self.assertEqual(str(RECIPE_PATH), outputs["recipe"])
        self.assertEqual("comfyui-python312-cu121", outputs["contract_id"])
        self.assertEqual("1.0.0", outputs["contract_version"])
        self.assertEqual("3.12", outputs["runtime_python_version"])
        self.assertEqual("linux-x86_64-cuda", outputs["runtime_platform"])
        self.assertEqual("https://download.pytorch.org/whl/cu121", outputs["pytorch_index_url"])
        self.assertEqual(
            ["torch==2.5.1", "torchvision==0.20.1", "torchaudio==2.5.1"],
            json.loads(outputs["pytorch_packages_json"]),
        )
        self.assertEqual(["requirements.txt"], json.loads(outputs["base_requirements_json"]))

    def test_rejects_recipe_fields_not_allowed_by_schema(self):
        with tempfile.TemporaryDirectory() as directory:
            recipe_path = Path(directory) / "recipe.yaml"
            shutil.copyfile(SCHEMA_PATH, Path(directory) / "schema.json")
            recipe_path.write_text(
                RECIPE_PATH.read_text(encoding="utf-8") + "\nmetadata:\n  default_implementation_revision: old\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(release_tool.ReleaseToolError, "metadata"):
                release_tool.load_recipe(recipe_path)

    def test_rejects_unsafe_base_requirement_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            recipe_path = Path(directory) / "recipe.yaml"
            shutil.copyfile(SCHEMA_PATH, Path(directory) / "schema.json")
            recipe_path.write_text(
                RECIPE_PATH.read_text(encoding="utf-8").replace(
                    "    - requirements.txt",
                    "    - ../requirements.txt",
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(release_tool.ReleaseToolError, "base requirement path is unsafe"):
                release_tool.load_recipe(recipe_path)

    def test_rejects_current_directory_base_requirement_path(self):
        with tempfile.TemporaryDirectory() as directory:
            recipe_path = Path(directory) / "recipe.yaml"
            shutil.copyfile(SCHEMA_PATH, Path(directory) / "schema.json")
            recipe_path.write_text(
                RECIPE_PATH.read_text(encoding="utf-8").replace(
                    "    - requirements.txt",
                    "    - .",
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(release_tool.ReleaseToolError, "base requirement path is unsafe"):
                release_tool.load_recipe(recipe_path)

    def test_dockerfile_keeps_recipe_build_inputs_without_runtime_identity_metadata(self):
        dockerfile = DOCKERFILE_PATH.read_text(encoding="utf-8")

        self.assertIn("LUMA_FORGE_BASE_REQUIREMENTS_JSON", dockerfile)
        self.assertIn("json.loads(os.environ[\"LUMA_FORGE_BASE_REQUIREMENTS_JSON\"])", dockerfile)
        self.assertIn("python -m venv --copies /opt/luma-forge/runtime/.venv", dockerfile)
        self.assertNotIn("LUMA_FORGE_RUNTIME_IMPLEMENTATION_REVISION", dockerfile)
        self.assertNotIn("LUMA_FORGE_PROVISIONER_IMAGE_REF", dockerfile)
        self.assertNotIn("runtime-contract.json", dockerfile)

    def test_workflow_has_no_manual_implementation_revision_input(self):
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")

        self.assertNotIn("implementation_revision:", workflow)
        self.assertNotIn("--implementation-revision", workflow)
        self.assertIn("--catalog bundled/runtime-catalog.json", workflow)
        self.assertIn("suffix=\"${CONTRACT_ID}-${CONTRACT_VERSION}\"", workflow)

    def test_catalog_validation_runs_before_build_or_publish(self):
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")

        validate_workers_index = workflow.index("Validate workers")
        build_provisioner_index = workflow.index("Build provisioner image")
        publish_index = workflow.index("Publish image pair")
        verify_catalog_pr_scope_index = workflow.index("Verify Runtime Catalog PR scope")
        catalog_pr_index = workflow.index("Open Runtime Catalog update PR")

        self.assertLess(validate_workers_index, build_provisioner_index)
        self.assertLess(validate_workers_index, publish_index)
        self.assertLess(verify_catalog_pr_scope_index, catalog_pr_index)

    def test_workflow_restricts_runtime_catalog_pr_to_catalog_file(self):
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")

        catalog_pr_section = workflow.split("Open Runtime Catalog update PR", maxsplit=1)[1]

        self.assertIn("add-paths:", catalog_pr_section)
        self.assertIn("bundled/runtime-catalog.json", catalog_pr_section)

    def test_workflow_fails_on_unexpected_catalog_pr_changes(self):
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        verify_section = workflow.split("Verify Runtime Catalog PR scope", maxsplit=1)[1].split(
            "Open Runtime Catalog update PR",
            maxsplit=1,
        )[0]

        self.assertIn("git status --porcelain --untracked-files=all", verify_section)
        self.assertIn("grep -vx 'bundled/runtime-catalog.json'", verify_section)
        self.assertIn("unexpected changed paths", verify_section)
        self.assertIn("exit 1", verify_section)

    def test_find_contract_matches_contract_id(self):
        recipe = release_tool.load_recipe(RECIPE_PATH)
        catalog = _catalog_with_contract(recipe, provisioner_ref=_image_ref("1"), endpoint_ref=_image_ref("2"))

        contract = release_tool.find_contract(catalog, "comfyui-python312-cu121")

        self.assertIsNotNone(contract)
        assert contract is not None
        revision = release_tool.find_revision(contract, "1.0.0")
        self.assertIsNotNone(revision)
        assert revision is not None
        self.assertEqual(_image_ref("1"), revision["provisioner_image_ref"])

    def test_update_catalog_creates_new_contract_from_image_refs(self):
        recipe = release_tool.load_recipe(RECIPE_PATH)
        catalog = {"contracts": []}

        updated = release_tool.update_catalog(
            recipe=recipe,
            catalog=catalog,
            provisioner_ref=_image_ref("1"),
            endpoint_ref=_image_ref("2"),
        )

        self.assertEqual(
            [
                {
                    "id": "comfyui-python312-cu121",
                    "revisions": [
                        {
                            "version": "1.0.0",
                            "provisioner_image_ref": _image_ref("1"),
                            "endpoint_image_ref": _image_ref("2"),
                        }
                    ],
                }
            ],
            updated["contracts"],
        )

    def test_update_catalog_replaces_existing_contract_image_refs(self):
        recipe = release_tool.load_recipe(RECIPE_PATH)
        catalog = _catalog_with_contract(recipe, provisioner_ref=_image_ref("1"), endpoint_ref=_image_ref("2"))

        updated = release_tool.update_catalog(
            recipe=recipe,
            catalog=catalog,
            provisioner_ref=_image_ref("3"),
            endpoint_ref=_image_ref("4"),
        )

        self.assertEqual(1, len(updated["contracts"]))
        self.assertEqual(1, len(updated["contracts"][0]["revisions"]))
        self.assertEqual(_image_ref("3"), updated["contracts"][0]["revisions"][0]["provisioner_image_ref"])
        self.assertEqual(_image_ref("4"), updated["contracts"][0]["revisions"][0]["endpoint_image_ref"])

    def test_update_catalog_rejects_mutable_image_refs(self):
        recipe = release_tool.load_recipe(RECIPE_PATH)
        catalog = {"contracts": []}

        with self.assertRaisesRegex(release_tool.ReleaseToolError, "digest-pinned"):
            release_tool.update_catalog(
                recipe=recipe,
                catalog=catalog,
                provisioner_ref="ghcr.io/luma-forge/test:latest",
                endpoint_ref=_image_ref("2"),
            )

    def test_bundled_catalog_uses_simplified_contract_entries(self):
        catalog = json.loads(CATALOG_PATH.read_text(encoding="utf-8"))

        self.assertIn("contracts", catalog)
        self.assertNotIn("runtime_contracts", catalog)
        self.assertEqual({"id", "revisions"}, set(catalog["contracts"][0]))
        self.assertEqual(
            {"version", "provisioner_image_ref", "endpoint_image_ref"},
            set(catalog["contracts"][0]["revisions"][0]),
        )
        release_tool.validate_catalog_compatibility(
            recipe=release_tool.load_recipe(RECIPE_PATH),
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
                    "--recipe",
                    str(RECIPE_PATH),
                    "--github-output",
                    str(output_path),
                ]
            )

            self.assertEqual(0, exit_code)
            output = output_path.read_text(encoding="utf-8")
            self.assertIn("pytorch_packages_json=", output)
            self.assertIn("contract_version=1.0.0", output)
            self.assertNotIn("implementation_revision=", output)

    def test_cli_update_catalog_writes_contract_image_refs(self):
        recipe = release_tool.load_recipe(RECIPE_PATH)
        with tempfile.TemporaryDirectory() as directory:
            catalog_path = Path(directory) / "runtime-catalog.json"
            catalog_path.write_text(
                json.dumps({"contracts": []}),
                encoding="utf-8",
            )

            exit_code = release_tool.main(
                [
                    "update-catalog",
                    "--recipe",
                    str(RECIPE_PATH),
                    "--catalog",
                    str(catalog_path),
                    "--provisioner-ref",
                    _image_ref("1"),
                    "--endpoint-ref",
                    _image_ref("2"),
                ]
            )

            self.assertEqual(0, exit_code)
            updated = json.loads(catalog_path.read_text(encoding="utf-8"))
            self.assertEqual("comfyui-python312-cu121", updated["contracts"][0]["id"])
            self.assertEqual("1.0.0", updated["contracts"][0]["revisions"][0]["version"])
            self.assertEqual(_image_ref("1"), updated["contracts"][0]["revisions"][0]["provisioner_image_ref"])


def _catalog_with_contract(recipe, *, provisioner_ref, endpoint_ref):
    return {
        "contracts": [
            {
                "id": recipe["contract"]["id"],
                "revisions": [
                    {
                        "version": recipe["contract"]["version"],
                        "provisioner_image_ref": provisioner_ref,
                        "endpoint_image_ref": endpoint_ref,
                    }
                ],
            }
        ],
    }


def _image_ref(seed):
    return f"ghcr.io/luma-forge/test@sha256:{seed * 64}"


if __name__ == "__main__":
    unittest.main()
