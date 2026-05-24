import json
import unittest
from pathlib import Path

from runpod_endpoint_worker.errors import WorkflowValidationError
from runpod_endpoint_worker.workflow import load_workflow, patch_hidream_workflow, validate_hidream_workflow


WORKFLOW_PATH = Path(__file__).resolve().parents[3] / "bundled/workflows/comfyui-hidream-o1-dev.json"


class HiDreamWorkflowTests(unittest.TestCase):
    def test_bundled_workflow_contains_expected_hidream_nodes(self):
        workflow = load_workflow(WORKFLOW_PATH)

        validate_hidream_workflow(workflow)

    def test_rejects_unexpected_hidream_node_shape(self):
        workflow = load_workflow(WORKFLOW_PATH)
        for node in workflow["nodes"]:
            if node["id"] == 171:
                node["type"] = "Unexpected"

        with self.assertRaises(WorkflowValidationError):
            validate_hidream_workflow(workflow)

    def test_patches_prompt_and_disables_image_edit_and_prompt_refine(self):
        workflow = load_workflow(WORKFLOW_PATH)

        patched = patch_hidream_workflow(workflow, "a glass lamp")

        values = {str(node["id"]): node["widgets_values"][0] for node in patched["nodes"] if node["id"] in (171, 154, 177)}
        self.assertEqual(values["171"], "a glass lamp")
        self.assertFalse(values["154"])
        self.assertFalse(values["177"])

        original_values = {str(node["id"]): node["widgets_values"][0] for node in workflow["nodes"] if node["id"] in (171, 154, 177)}
        self.assertNotEqual(original_values["171"], "a glass lamp")


if __name__ == "__main__":
    unittest.main()
