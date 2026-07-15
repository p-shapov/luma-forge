import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
TOOL_PATH = ROOT / "workers/provisioner/release_tool.py"
PROVISIONER_WORKFLOW_PATH = ROOT / ".github/workflows/deploy-provisioner.yml"

spec = importlib.util.spec_from_file_location("provisioner_promotion_tool", TOOL_PATH)
release_tool = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(release_tool)


class ProvisionerPromotionToolTests(unittest.TestCase):
    def test_provisioner_workflow_promotes_digest_after_publish(self):
        workflow = PROVISIONER_WORKFLOW_PATH.read_text(encoding="utf-8")

        resolve_index = workflow.index("Resolve provisioner metadata")
        tag_index = workflow.index("Resolve image tag")
        publish_index = workflow.index("Publish provisioner image")
        digest_index = workflow.index("Resolve pushed image digest")
        promotion_index = workflow.index("Promote provisioner image to catalog")
        promotion_section = workflow.split(
            "Promote provisioner image to catalog", maxsplit=1
        )[1].split("Verify provisioner promotion PR scope", maxsplit=1)[0]

        self.assertLess(resolve_index, tag_index)
        self.assertLess(publish_index, digest_index)
        self.assertLess(digest_index, promotion_index)
        self.assertIn("docker inspect --format='{{index .RepoDigests 0}}'", workflow)
        self.assertIn("--catalog-root bundled/catalog", workflow)
        self.assertIn(
            "workers/provisioner/release_tool.py promote-provisioner-image",
            promotion_section,
        )
        self.assertIn(
            '--contract-revision "${{ steps.contract.outputs.contract_revision }}"',
            promotion_section,
        )
        self.assertIn(
            '--image-ref "${{ steps.digest.outputs.provisioner_ref }}"',
            promotion_section,
        )
        self.assertIn('--github-output "$GITHUB_OUTPUT"', promotion_section)

    def test_provisioner_workflow_restricts_catalog_promotion_pr_scope(self):
        workflow = PROVISIONER_WORKFLOW_PATH.read_text(encoding="utf-8")
        verify_section = workflow.split(
            "Verify provisioner promotion PR scope", maxsplit=1
        )[1].split("Open provisioner promotion PR", maxsplit=1)[0]
        pr_section = workflow.split("Open provisioner promotion PR", maxsplit=1)[1]

        self.assertIn(
            'porcelain_entries="$(git status --porcelain --untracked-files=all)"',
            verify_section,
        )
        self.assertIn(
            'runtime_contract_entry="?? $runtime_contract_path"', verify_section
        )
        self.assertIn(
            "bundled/catalog/entries/runtime_contracts/provisioner/${{ steps.contract.outputs.contract_revision }}/runtime_contract",
            verify_section,
        )
        self.assertIn(
            r"\?\? bundled/catalog/entries/workflows/[a-z][a-z0-9-]*/(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)",
            verify_section,
        )
        self.assertIn(
            "(metadata|model_assets|contract_requirements|execution_contract|workflow)",
            verify_section,
        )
        self.assertIn("unexpected changed paths", verify_section)
        self.assertIn("add-paths:", pr_section)
        self.assertIn("${{ steps.promotion.outputs.runtime_contract_path }}", pr_section)
        self.assertIn("bundled/catalog/entries/workflows", pr_section)
        self.assertIn(
            "branch: provisioners/${{ steps.contract.outputs.contract_id }}-${{ steps.contract.outputs.contract_revision }}",
            pr_section,
        )
        self.assertIn(
            'commit-message: "chore(workers): promote provisioner image ${{ steps.contract.outputs.contract_id }} ${{ steps.contract.outputs.contract_revision }}"',
            pr_section,
        )

    def test_next_provisioner_contract_revision_uses_directory_names(self):
        with tempfile.TemporaryDirectory() as directory:
            catalog_root = _write_catalog_tree(Path(directory))
            self.assertEqual(
                "1.0.1",
                release_tool.next_provisioner_contract_revision(
                    catalog_root, "provisioner"
                ),
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
            self.assertEqual(
                {"image_ref": _image_ref("4")},
                json.loads(contract_path.read_text()),
            )
            self.assertEqual(
                [catalog_root / "entries/workflows/workflow/1.0.1"], workflows
            )
            promoted = json.loads(
                (workflows[0] / "contract_requirements").read_text()
            )
            self.assertEqual(
                "1.0.1",
                promoted["contract_requirements"][0][
                    "provisioner_contract_ref"
                ]["revision"],
            )
            self.assertEqual(
                "1.0.0",
                json.loads(
                    (
                        catalog_root
                        / "entries/workflows/workflow/1.0.0/contract_requirements"
                    ).read_text()
                )["contract_requirements"][0]["provisioner_contract_ref"][
                    "revision"
                ],
            )

    def test_promote_rejects_mutable_image_ref(self):
        with tempfile.TemporaryDirectory() as directory:
            catalog_root = _write_catalog_tree(Path(directory))

            with self.assertRaisesRegex(release_tool.ReleaseToolError, "digest-pinned"):
                release_tool.promote_provisioner_image(
                    catalog_root=catalog_root,
                    contract_id="provisioner",
                    contract_revision="1.0.1",
                    image_ref="ghcr.io/luma-forge/provisioner-worker:latest",
                )

    def test_promote_rejects_wrong_provisioner_contract(self):
        with tempfile.TemporaryDirectory() as directory:
            catalog_root = _write_catalog_tree(Path(directory))
            requirements_path = (
                catalog_root / "entries/workflows/workflow/1.0.0/contract_requirements"
            )
            requirements = json.loads(requirements_path.read_text(encoding="utf-8"))
            requirements["contract_requirements"][0]["provisioner_contract_ref"][
                "contract"
            ] = "catalog/contracts/runtime_preset_revision"
            requirements_path.write_text(json.dumps(requirements), encoding="utf-8")

            with self.assertRaisesRegex(
                release_tool.ReleaseToolError, "uses an unexpected contract"
            ):
                release_tool.promote_provisioner_image(
                    catalog_root=catalog_root,
                    contract_id="provisioner",
                    contract_revision="1.0.1",
                    image_ref=_image_ref("4"),
                )

    def test_promote_rejects_dangling_current_provisioner_contract_revision(self):
        with tempfile.TemporaryDirectory() as directory:
            catalog_root = _write_catalog_tree(Path(directory))
            requirements_path = (
                catalog_root / "entries/workflows/workflow/1.0.0/contract_requirements"
            )
            requirements = json.loads(requirements_path.read_text(encoding="utf-8"))
            requirements["contract_requirements"][0]["provisioner_contract_ref"][
                "revision"
            ] = "9.9.9"
            requirements_path.write_text(json.dumps(requirements), encoding="utf-8")

            with self.assertRaisesRegex(
                release_tool.ReleaseToolError, "catalog entry file does not exist"
            ):
                release_tool.promote_provisioner_image(
                    catalog_root=catalog_root,
                    contract_id="provisioner",
                    contract_revision="1.0.1",
                    image_ref=_image_ref("4"),
                )

    def test_promote_rejects_unsafe_workflow_family_id_before_writing(self):
        with tempfile.TemporaryDirectory() as directory:
            catalog_root = _write_catalog_tree(Path(directory))
            workflow_root = catalog_root / "entries/workflows"
            (workflow_root / "workflow").rename(workflow_root / "unsafe_id")
            contract_dir = (
                catalog_root / "entries/runtime_contracts/provisioner/1.0.1"
            )

            with self.assertRaisesRegex(
                release_tool.ReleaseToolError, "invalid workflow id"
            ):
                release_tool.promote_provisioner_image(
                    catalog_root=catalog_root,
                    contract_id="provisioner",
                    contract_revision="1.0.1",
                    image_ref=_image_ref("4"),
                )

            self.assertFalse(contract_dir.exists())

    def test_promote_rejects_symlinked_workflow_file_before_writing(self):
        with tempfile.TemporaryDirectory() as directory:
            catalog_root = _write_catalog_tree(Path(directory))
            source = catalog_root / "entries/workflows/workflow/1.0.0"
            metadata = source / "metadata"
            metadata.unlink()
            metadata.symlink_to(
                catalog_root
                / "entries/runtime_contracts/provisioner/1.0.0/runtime_contract"
            )
            contract_dir = (
                catalog_root / "entries/runtime_contracts/provisioner/1.0.1"
            )

            with self.assertRaisesRegex(
                release_tool.ReleaseToolError, "must not be a symlink"
            ):
                release_tool.promote_provisioner_image(
                    catalog_root=catalog_root,
                    contract_id="provisioner",
                    contract_revision="1.0.1",
                    image_ref=_image_ref("4"),
                )

            self.assertFalse(contract_dir.exists())

    def test_promote_rejects_symlinked_workflow_family_before_writing(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            catalog_root = _write_catalog_tree(root)
            workflow_family = catalog_root / "entries/workflows/workflow"
            outside_family = root / "outside-workflow"
            workflow_family.rename(outside_family)
            workflow_family.symlink_to(outside_family, target_is_directory=True)
            contract_destination = (
                catalog_root / "entries/runtime_contracts/provisioner/1.0.1"
            )
            workflow_destination = outside_family / "1.0.1"

            with self.assertRaisesRegex(
                release_tool.ReleaseToolError, "must not be a symlink"
            ):
                release_tool.promote_provisioner_image(
                    catalog_root=catalog_root,
                    contract_id="provisioner",
                    contract_revision="1.0.1",
                    image_ref=_image_ref("4"),
                )

            self.assertFalse(contract_destination.exists())
            self.assertFalse(workflow_destination.exists())

    def test_promote_rejects_dangling_workflow_destination_before_writing(self):
        with tempfile.TemporaryDirectory() as directory:
            catalog_root = _write_catalog_tree(Path(directory))
            destination = catalog_root / "entries/workflows/workflow/1.0.1"
            destination.symlink_to(
                Path(directory) / "missing", target_is_directory=True
            )
            contract_dir = (
                catalog_root / "entries/runtime_contracts/provisioner/1.0.1"
            )

            with self.assertRaisesRegex(
                release_tool.ReleaseToolError, "must not be a symlink"
            ):
                release_tool.promote_provisioner_image(
                    catalog_root=catalog_root,
                    contract_id="provisioner",
                    contract_revision="1.0.1",
                    image_ref=_image_ref("4"),
                )

            self.assertFalse(contract_dir.exists())

    def test_promote_rejects_dangling_contract_destination_before_writing(self):
        with tempfile.TemporaryDirectory() as directory:
            catalog_root = _write_catalog_tree(Path(directory))
            contract_dir = (
                catalog_root / "entries/runtime_contracts/provisioner/1.0.1"
            )
            contract_dir.symlink_to(
                Path(directory) / "missing", target_is_directory=True
            )
            workflow_destination = (
                catalog_root / "entries/workflows/workflow/1.0.1"
            )

            with self.assertRaisesRegex(
                release_tool.ReleaseToolError, "destination already exists"
            ):
                release_tool.promote_provisioner_image(
                    catalog_root=catalog_root,
                    contract_id="provisioner",
                    contract_revision="1.0.1",
                    image_ref=_image_ref("4"),
                )

            self.assertFalse(workflow_destination.exists())

    def test_cli_writes_revision_outputs(self):
        with tempfile.TemporaryDirectory(dir=ROOT) as directory:
            catalog_root = _write_catalog_tree(Path(directory)).relative_to(ROOT)
            resolve_output = Path(directory) / "resolve-output"
            promotion_output = Path(directory) / "promotion-output"

            self.assertEqual(
                0,
                release_tool.main(
                    [
                        "resolve-provisioner",
                        "--catalog-root",
                        str(catalog_root),
                        "--github-output",
                        str(resolve_output),
                    ]
                ),
            )
            self.assertEqual(
                {
                    "contract_id": "provisioner",
                    "contract_revision": "1.0.1",
                },
                _read_outputs(resolve_output),
            )

            self.assertEqual(
                0,
                release_tool.main(
                    [
                        "promote-provisioner-image",
                        "--catalog-root",
                        str(catalog_root),
                        "--contract-revision",
                        "1.0.1",
                        "--image-ref",
                        _image_ref("4"),
                        "--github-output",
                        str(promotion_output),
                    ]
                ),
            )
            self.assertEqual(
                str(
                    catalog_root
                    / "entries/runtime_contracts/provisioner/1.0.1/runtime_contract"
                ),
                _read_outputs(promotion_output)["runtime_contract_path"],
            )


def _write_catalog_tree(root: Path) -> Path:
    catalog_root = root / "catalog"
    contract = (
        catalog_root
        / "entries/runtime_contracts/provisioner/1.0.0/runtime_contract"
    )
    contract.parent.mkdir(parents=True)
    contract.write_text(json.dumps({"image_ref": _image_ref("2")}), encoding="utf-8")

    workflow = catalog_root / "entries/workflows/workflow/1.0.0"
    workflow.mkdir(parents=True)
    documents = {
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
    for name, value in documents.items():
        (workflow / name).write_text(json.dumps(value), encoding="utf-8")
    return catalog_root


def _read_outputs(path: Path) -> dict[str, str]:
    return dict(
        line.split("=", 1)
        for line in path.read_text(encoding="utf-8").splitlines()
    )


def _image_ref(seed: str) -> str:
    return f"ghcr.io/luma-forge/test@sha256:{seed * 64}"


if __name__ == "__main__":
    unittest.main()
