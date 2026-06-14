import json
import tempfile
import unittest
from pathlib import Path

from app.errors import WorkflowValidationError
from runtime.workflow import apply_input_bindings, load_workflow, write_patched_workflow


WORKFLOW_PATH = Path(__file__).resolve().parents[3] / "bundled/workflows/comfyui-hidream-o1-dev.json"


def contract():
    return {
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
    }


class WorkflowBindingTests(unittest.TestCase):
    def test_applies_template_and_literal_bindings(self):
        workflow = load_workflow(WORKFLOW_PATH)

        patched = apply_input_bindings(workflow, contract(), {"prompt": "a glass lamp"})

        values = {str(node["id"]): node["widgets_values"][0] for node in patched["nodes"] if node["id"] in (171, 154, 177)}
        self.assertEqual(values["171"], "a glass lamp")
        self.assertFalse(values["154"])
        self.assertFalse(values["177"])

        original_values = {str(node["id"]): node["widgets_values"][0] for node in workflow["nodes"] if node["id"] in (171, 154, 177)}
        self.assertNotEqual(original_values["171"], "a glass lamp")

    def test_treats_non_template_string_as_literal_constant(self):
        workflow = {
            "nodes": [
                {"id": 1, "widgets_values": ["old"]},
            ]
        }
        patched = apply_input_bindings(
            workflow,
            {
                "input_bindings": [{"value": "literal prompt", "node_id": "1", "path": ["widgets_values", "0"]}],
            },
            {"prompt": "ignored"},
        )

        self.assertEqual(patched["nodes"][0]["widgets_values"][0], "literal prompt")

    def test_rejects_missing_node(self):
        workflow = {"nodes": [{"id": 1, "widgets_values": ["old"]}]}

        with self.assertRaises(WorkflowValidationError):
            apply_input_bindings(
                workflow,
                {
                    "input_bindings": [{"value": "{{prompt}}", "node_id": "2", "path": ["widgets_values", "0"]}],
                },
                {"prompt": "a lamp"},
            )

    def test_rejects_invalid_path(self):
        workflow = {"nodes": [{"id": 1, "widgets_values": []}]}

        with self.assertRaises(WorkflowValidationError):
            apply_input_bindings(
                workflow,
                {
                    "input_bindings": [{"value": "{{prompt}}", "node_id": "1", "path": ["widgets_values", "0"]}],
                },
                {"prompt": "a lamp"},
            )

    def test_write_patched_workflow_uses_contract_file(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "workflow.json"
            destination = Path(directory) / "patched.json"
            contract_path = Path(directory) / "execution-contract.json"
            source.write_text(json.dumps({"nodes": [{"id": 1, "widgets_values": ["old"]}]}), encoding="utf-8")
            contract_path.write_text(
                json.dumps(
                    {
                        "input_bindings": [{"value": "{{prompt}}", "node_id": "1", "path": ["widgets_values", "0"]}],
                    }
                ),
                encoding="utf-8",
            )

            write_patched_workflow(source, contract_path, destination, {"prompt": "a lamp"})

            patched = json.loads(destination.read_text(encoding="utf-8"))
            self.assertEqual(patched["nodes"][0]["widgets_values"][0], "a lamp")


if __name__ == "__main__":
    unittest.main()
