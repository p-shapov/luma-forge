import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
TOOL_PATH = ROOT / "workers/promote-provisioner/release_tool.py"
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
        promotion_section = workflow.split("Promote provisioner image to catalog", maxsplit=1)[1].split(
            "Verify provisioner promotion PR scope",
            maxsplit=1,
        )[0]

        self.assertLess(resolve_index, tag_index)
        self.assertLess(publish_index, digest_index)
        self.assertLess(digest_index, promotion_index)
        self.assertIn("docker inspect --format='{{index .RepoDigests 0}}'", workflow)
        self.assertIn("workers/promote-provisioner/release_tool.py resolve-provisioner", workflow)
        self.assertIn(
            "workers/promote-provisioner/release_tool.py promote-provisioner-image",
            promotion_section,
        )
        self.assertIn("--image-ref \"${{ steps.digest.outputs.provisioner_ref }}\"", promotion_section)

    def test_provisioner_workflow_restricts_catalog_promotion_pr_to_catalog_files(self):
        workflow = PROVISIONER_WORKFLOW_PATH.read_text(encoding="utf-8")
        verify_section = workflow.split("Verify provisioner promotion PR scope", maxsplit=1)[1].split(
            "Open provisioner promotion PR",
            maxsplit=1,
        )[0]
        pr_section = workflow.split("Open provisioner promotion PR", maxsplit=1)[1]

        self.assertIn("git status --porcelain --untracked-files=all", verify_section)
        self.assertIn("grep -Evx 'bundled/(runtime-contracts|workflow-catalog)\\.json'", verify_section)
        self.assertIn("unexpected changed paths", verify_section)
        self.assertIn("add-paths:", pr_section)
        self.assertIn("bundled/runtime-contracts.json", pr_section)
        self.assertIn("bundled/workflow-catalog.json", pr_section)
        self.assertIn("promote provisioner image", pr_section)
        self.assertIn(
            "branch: provisioners/${{ steps.contract.outputs.contract_id }}-${{ steps.contract.outputs.contract_version }}",
            pr_section,
        )
        self.assertIn(
            'commit-message: "chore(workers): promote provisioner image ${{ steps.contract.outputs.contract_id }} ${{ steps.contract.outputs.contract_version }}"',
            pr_section,
        )
        self.assertIn("Published image: `${{ steps.digest.outputs.provisioner_ref }}`", pr_section)
        self.assertIn("Catalog version: `${{ steps.contract.outputs.contract_version }}`", pr_section)

    def test_next_provisioner_contract_version_uses_next_patch(self):
        catalog = _provisioner_catalog()

        version = release_tool.next_provisioner_contract_version(
            catalog=catalog,
            contract_id="provisioner",
        )

        self.assertEqual("1.0.1", version)

    def test_promote_provisioner_image_appends_revision(self):
        catalog = _provisioner_catalog()

        updated = release_tool.promote_provisioner_image(
            catalog=catalog,
            contract_id="provisioner",
            image_ref=_image_ref("4"),
        )

        revisions = updated["contracts"][0]["revisions"]
        self.assertEqual(2, len(revisions))
        self.assertEqual("1.0.0", revisions[0]["version"])
        self.assertEqual("1.0.1", revisions[1]["version"])
        self.assertEqual(_image_ref("4"), revisions[1]["image_ref"])

    def test_promote_provisioner_image_rejects_duplicate_explicit_revision(self):
        catalog = _provisioner_catalog()

        with self.assertRaisesRegex(release_tool.ReleaseToolError, "already exists"):
            release_tool.promote_provisioner_image(
                catalog=catalog,
                contract_id="provisioner",
                image_ref=_image_ref("4"),
                contract_version="1.0.0",
            )

    def test_promote_provisioner_image_rejects_mutable_image_refs(self):
        catalog = _provisioner_catalog()

        with self.assertRaisesRegex(release_tool.ReleaseToolError, "digest-pinned"):
            release_tool.promote_provisioner_image(
                catalog=catalog,
                contract_id="provisioner",
                image_ref="ghcr.io/luma-forge/provisioner-worker:latest",
            )

    def test_promote_provisioner_image_updates_workflow_catalog(self):
        workflow_catalog = _workflow_catalog()

        updated = release_tool.update_provisioner_workflow_catalog(
            catalog=workflow_catalog,
            contract_id="provisioner",
            contract_version="1.0.1",
        )

        self.assertEqual(
            "1.0.1",
            updated["workflow_presets"][0]["revisions"][0]["contract_requirements"][0]["provisioner_contract"][
                "version"
            ],
        )

    def test_cli_resolve_provisioner_writes_next_catalog_revision(self):
        with tempfile.TemporaryDirectory() as directory:
            catalog_path = Path(directory) / "runtime-contracts.json"
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
            self.assertIn("contract_id=provisioner", output)
            self.assertIn("contract_version=1.0.1", output)

    def test_cli_promote_provisioner_image_appends_revision_and_updates_workflow_catalog(self):
        with tempfile.TemporaryDirectory() as directory:
            catalog_path = Path(directory) / "runtime-contracts.json"
            workflow_path = Path(directory) / "workflow-catalog.json"
            catalog_path.write_text(json.dumps(_provisioner_catalog()), encoding="utf-8")
            workflow_path.write_text(json.dumps(_workflow_catalog()), encoding="utf-8")

            exit_code = release_tool.main(
                [
                    "promote-provisioner-image",
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
                    "provisioner_contract"
                ]["version"],
            )

    def test_provisioner_contracts_reject_malformed_catalog(self):
        with self.assertRaisesRegex(release_tool.ReleaseToolError, "contracts must be a list"):
            release_tool.next_provisioner_contract_version(
                catalog={"contracts": {}},
                contract_id="provisioner",
            )


def _provisioner_catalog():
    return {
        "contracts": [
            {
                "id": "provisioner",
                "revisions": [
                    {
                        "version": "1.0.0",
                        "image_ref": _image_ref("2"),
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
                "revisions": [
                    {
                        "version": "1.0.0",
                        "runtime_preset": "comfyui-py312-cu126-torch291",
                        "contract_requirements": [
                            {
                                "runtime_type": "runpod",
                                "endpoint_contract": {
                                    "id": "runpod-endpoint-preset",
                                    "version": "1.0.0",
                                },
                                "provisioner_contract": {
                                    "id": "provisioner",
                                    "version": "1.0.0",
                                },
                            }
                        ],
                    }
                ],
            }
        ]
    }


def _image_ref(seed):
    return f"ghcr.io/luma-forge/test@sha256:{seed * 64}"


if __name__ == "__main__":
    unittest.main()
