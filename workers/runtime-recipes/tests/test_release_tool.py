import copy
import importlib.util
import json
import shutil
import tarfile
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
    def test_normalizes_recipe_compatibility_metadata(self):
        recipe = release_tool.load_recipe(RECIPE_PATH)

        compatibility = release_tool.compatibility_metadata_from_recipe(recipe)

        self.assertEqual("image_baked_comfyui_runtime", compatibility["environment_kind"])
        self.assertEqual("3.12", compatibility["python_version"])
        self.assertEqual("linux-x86_64-cuda", compatibility["platform"])
        self.assertEqual(
            "aa9d2fc713664e9ffe37763f4c9240c0c3eda667",
            compatibility["comfyui_revision"],
        )
        self.assertEqual("https://download.pytorch.org/whl/cu121", compatibility["pytorch_index_url"])
        self.assertEqual(
            ["torch==2.5.1", "torchvision==0.20.1", "torchaudio==2.5.1"],
            compatibility["pytorch_packages"],
        )
        self.assertEqual(["requirements.txt"], compatibility["base_requirements"])

    def test_recipe_outputs_include_pytorch_build_arguments(self):
        recipe = release_tool.load_recipe(RECIPE_PATH)

        outputs = release_tool.recipe_outputs(recipe, RECIPE_PATH, "2026.05.17-001")

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
                RECIPE_PATH.read_text(encoding="utf-8") + "\nunsupported: true\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(release_tool.ReleaseToolError, "unsupported"):
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

    def test_dockerfile_installs_recipe_base_requirements(self):
        dockerfile = DOCKERFILE_PATH.read_text(encoding="utf-8")

        self.assertIn("LUMA_FORGE_BASE_REQUIREMENTS_JSON", dockerfile)
        self.assertIn("json.loads(os.environ[\"LUMA_FORGE_BASE_REQUIREMENTS_JSON\"])", dockerfile)
        self.assertNotIn("-r /workspace/ComfyUI/requirements.txt", dockerfile)

    def test_dockerfile_builds_runtime_venv_without_symlinks(self):
        dockerfile = DOCKERFILE_PATH.read_text(encoding="utf-8")

        self.assertIn("python -m venv --copies /workspace/.venv", dockerfile)

    def test_manual_dispatch_allows_optional_auto_revision_without_stale_default(self):
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        implementation_revision_section = workflow.split("implementation_revision:", maxsplit=1)[1].split(
            "\n\npermissions:",
            maxsplit=1,
        )[0]

        self.assertIn("required: false", implementation_revision_section)
        self.assertNotIn("default:", implementation_revision_section)
        self.assertNotIn("2026.05.16-001", implementation_revision_section)
        self.assertIn("--catalog bundled/runtime-catalog.json", workflow)

    def test_resolves_first_auto_implementation_revision_for_today(self):
        recipe = release_tool.load_recipe(RECIPE_PATH)
        catalog = _catalog_with_contract(recipe, implementation_revision="2026.05.16-001")

        revision = release_tool.resolve_implementation_revision(
            recipe=recipe,
            catalog=catalog,
            requested_revision="auto",
            today=release_tool.dt.date(2026, 5, 17),
        )

        self.assertEqual("2026.05.17-001", revision)

    def test_resolves_next_auto_implementation_revision_for_today(self):
        recipe = release_tool.load_recipe(RECIPE_PATH)
        catalog = _catalog_with_contract(
            recipe,
            implementation_revision=["2026.05.17-001", "2026.05.17-002", "manual-hotfix"],
        )

        revision = release_tool.resolve_implementation_revision(
            recipe=recipe,
            catalog=catalog,
            requested_revision="",
            today=release_tool.dt.date(2026, 5, 17),
        )

        self.assertEqual("2026.05.17-003", revision)

    def test_explicit_implementation_revision_is_preserved(self):
        recipe = release_tool.load_recipe(RECIPE_PATH)
        catalog = _catalog_with_contract(recipe, implementation_revision="2026.05.17-001")

        revision = release_tool.resolve_implementation_revision(
            recipe=recipe,
            catalog=catalog,
            requested_revision="manual-hotfix",
            today=release_tool.dt.date(2026, 5, 17),
        )

        self.assertEqual("manual-hotfix", revision)

    def test_catalog_validation_runs_before_build_or_publish(self):
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")

        validate_catalog_index = workflow.index("Validate runtime catalog compatibility")
        validate_workers_index = workflow.index("Validate workers")
        build_provisioner_index = workflow.index("Build provisioner image")
        publish_index = workflow.index("Publish image pair")
        verify_catalog_pr_scope_index = workflow.index("Verify Runtime Catalog PR scope")
        catalog_pr_index = workflow.index("Open Runtime Catalog update PR")

        self.assertLess(validate_catalog_index, validate_workers_index)
        self.assertLess(validate_catalog_index, build_provisioner_index)
        self.assertLess(validate_catalog_index, publish_index)
        self.assertLess(verify_catalog_pr_scope_index, catalog_pr_index)
        self.assertLess(validate_catalog_index, catalog_pr_index)

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

    def test_accepts_compatible_existing_contract_append(self):
        recipe = release_tool.load_recipe(RECIPE_PATH)
        catalog = _catalog_with_contract(recipe, implementation_revision="2026.05.16-001")

        release_tool.validate_catalog_compatibility(
            recipe=recipe,
            catalog=catalog,
            implementation_revision="2026.05.17-001",
        )

    def test_rejects_incompatible_existing_contract_append(self):
        recipe = release_tool.load_recipe(RECIPE_PATH)
        catalog = _catalog_with_contract(recipe, implementation_revision="2026.05.16-001")
        catalog["runtime_contracts"][0]["runtime_metadata"]["runtime_compatibility"]["pytorch_packages"] = [
            "torch==2.6.0"
        ]

        with self.assertRaisesRegex(
            release_tool.ReleaseToolError,
            "pytorch_packages.*Bump the runtime contract version",
        ):
            release_tool.validate_catalog_compatibility(
                recipe=recipe,
                catalog=catalog,
                implementation_revision="2026.05.17-001",
            )

    def test_rejects_duplicate_existing_implementation_revision(self):
        recipe = release_tool.load_recipe(RECIPE_PATH)
        catalog = _catalog_with_contract(recipe, implementation_revision="2026.05.16-001")

        with self.assertRaisesRegex(release_tool.ReleaseToolError, "implementation revision already exists"):
            release_tool.validate_catalog_compatibility(
                recipe=recipe,
                catalog=catalog,
                implementation_revision="2026.05.16-001",
            )

    def test_bundled_catalog_rejects_cataloged_implementation_revision(self):
        recipe = release_tool.load_recipe(RECIPE_PATH)
        catalog = json.loads(CATALOG_PATH.read_text(encoding="utf-8"))

        with self.assertRaisesRegex(
            release_tool.ReleaseToolError,
            "implementation revision already exists: 2026.05.16-001",
        ):
            release_tool.validate_catalog_compatibility(
                recipe=recipe,
                catalog=catalog,
                implementation_revision="2026.05.16-001",
            )

    def test_bundled_catalog_accepts_fresh_implementation_revision(self):
        recipe = release_tool.load_recipe(RECIPE_PATH)
        catalog = json.loads(CATALOG_PATH.read_text(encoding="utf-8"))

        release_tool.validate_catalog_compatibility(
            recipe=recipe,
            catalog=catalog,
            implementation_revision="2026.05.17-001",
        )

    def test_update_catalog_creates_new_contract_from_recipe_metadata(self):
        recipe = release_tool.load_recipe(RECIPE_PATH)
        catalog = {"id": "luma-forge-runtimes", "version": "2026.05.16", "runtime_contracts": []}

        updated = release_tool.update_catalog(
            recipe=recipe,
            catalog=catalog,
            implementation_revision="2026.05.17-001",
            provisioner_ref=_image_ref("1"),
            endpoint_ref=_image_ref("2"),
        )

        contract = updated["runtime_contracts"][0]
        self.assertEqual("comfyui-python312-cu121", contract["id"])
        self.assertEqual("1.0.0", contract["version"])
        self.assertEqual(
            ["torch==2.5.1", "torchvision==0.20.1", "torchaudio==2.5.1"],
            contract["runtime_metadata"]["runtime_compatibility"]["pytorch_packages"],
        )
        self.assertEqual("2026.05.17-001", contract["default_implementation_revision"])
        self.assertEqual(_image_ref("1"), contract["implementation_revisions"][0]["provisioner_image_ref"])

    def test_validate_image_metadata_rejects_recipe_drift(self):
        recipe = release_tool.load_recipe(RECIPE_PATH)
        provisioner_metadata = _provisioner_metadata(recipe, "2026.05.17-001")
        endpoint_metadata = _endpoint_metadata(recipe, "2026.05.17-001")
        provisioner_metadata["pytorch_packages"] = ["torch==2.6.0"]

        with self.assertRaisesRegex(release_tool.ReleaseToolError, "pytorch_packages"):
            release_tool.validate_image_metadata(
                recipe=recipe,
                implementation_revision="2026.05.17-001",
                provisioner_metadata=provisioner_metadata,
                endpoint_metadata=endpoint_metadata,
            )

    def test_validate_runtime_archive_rejects_absolute_symlinks(self):
        with tempfile.TemporaryDirectory() as directory:
            archive_path = Path(directory) / "base-runtime.tar.gz"
            with tarfile.open(archive_path, "w:gz") as archive:
                link = tarfile.TarInfo(".venv/bin/python3.12")
                link.type = tarfile.SYMTYPE
                link.linkname = "/usr/local/bin/python3.12"
                archive.addfile(link)

            with self.assertRaisesRegex(release_tool.ReleaseToolError, "safely extractable"):
                release_tool.validate_runtime_archive(archive_path)

    def test_validate_runtime_archive_accepts_data_filter_safe_members(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            payload = root / "payload"
            payload.mkdir()
            (payload / "main.py").write_text("# ComfyUI\n", encoding="utf-8")
            archive_path = root / "base-runtime.tar.gz"
            with tarfile.open(archive_path, "w:gz") as archive:
                archive.add(payload / "main.py", arcname="ComfyUI/main.py")

            release_tool.validate_runtime_archive(archive_path)

    def test_cli_writes_github_outputs(self):
        with tempfile.TemporaryDirectory() as directory:
            output_path = Path(directory) / "github-output"

            exit_code = release_tool.main(
                [
                    "resolve",
                    "--recipe",
                    str(RECIPE_PATH),
                    "--implementation-revision",
                    "2026.05.17-001",
                    "--github-output",
                    str(output_path),
                ]
            )

            self.assertEqual(0, exit_code)
            self.assertIn("pytorch_packages_json=", output_path.read_text(encoding="utf-8"))

    def test_cli_resolves_auto_implementation_revision_from_catalog(self):
        with tempfile.TemporaryDirectory() as directory:
            output_path = Path(directory) / "github-output"

            exit_code = release_tool.main(
                [
                    "resolve",
                    "--recipe",
                    str(RECIPE_PATH),
                    "--catalog",
                    str(CATALOG_PATH),
                    "--implementation-revision",
                    "auto",
                    "--github-output",
                    str(output_path),
                ]
            )

            self.assertEqual(0, exit_code)
            self.assertRegex(
                output_path.read_text(encoding="utf-8"),
                r"implementation_revision=\d{4}\.\d{2}\.\d{2}-\d{3}",
            )


def _catalog_with_contract(recipe, *, implementation_revision):
    implementation_revisions = (
        implementation_revision if isinstance(implementation_revision, list) else [implementation_revision]
    )
    return {
        "id": "luma-forge-runtimes",
        "version": "2026.05.16",
        "runtime_contracts": [
            {
                "id": recipe["contract"]["id"],
                "version": recipe["contract"]["version"],
                "display_name": "ComfyUI Python 3.12 CUDA 12.1 Runtime",
                "runtime_metadata": release_tool.runtime_metadata_from_recipe(recipe),
                "implementation_revisions": [
                    {
                        "revision": revision,
                        "provisioner_image_ref": _image_ref("1"),
                        "endpoint_image_ref": _image_ref("2"),
                        "image_metadata": {
                            "provisioner_runtime_archive_path": release_tool.PROVISIONER_RUNTIME_ARCHIVE_PATH,
                            "provisioner_runtime_metadata_path": release_tool.PROVISIONER_RUNTIME_METADATA_PATH,
                            "endpoint_runtime_contract_path": release_tool.ENDPOINT_RUNTIME_CONTRACT_PATH,
                        },
                    }
                    for revision in implementation_revisions
                ],
                "default_implementation_revision": implementation_revisions[-1],
            }
        ],
    }


def _provisioner_metadata(recipe, implementation_revision):
    metadata = {
        "contract_id": recipe["contract"]["id"],
        "contract_version": recipe["contract"]["version"],
        "implementation_revision": implementation_revision,
    }
    compatibility = release_tool.compatibility_metadata_from_recipe(recipe)
    metadata.update(
        {
            "python_version": compatibility["python_version"],
            "platform": compatibility["platform"],
            "comfyui_revision": compatibility["comfyui_revision"],
            "pytorch_index_url": compatibility["pytorch_index_url"],
            "pytorch_packages": copy.deepcopy(compatibility["pytorch_packages"]),
            "base_requirements": copy.deepcopy(compatibility["base_requirements"]),
        }
    )
    return metadata


def _endpoint_metadata(recipe, implementation_revision):
    return {
        "contract_id": recipe["contract"]["id"],
        "contract_version": recipe["contract"]["version"],
        "implementation_revision": implementation_revision,
    }


def _image_ref(seed):
    return f"ghcr.io/luma-forge/test@sha256:{seed * 64}"


if __name__ == "__main__":
    unittest.main()
