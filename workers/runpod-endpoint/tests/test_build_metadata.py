import json
import tempfile
import unittest
from pathlib import Path

from tools.build_metadata import extract_runtime_metadata


class BuildMetadataTests(unittest.TestCase):
    def test_extracts_execution_contract_from_direct_documents(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source_contract = root / "execution_contract"
            source_schema = root / "execution_schema"
            output = root / "runtime_execution_contract.json"
            source_contract.write_text(
                json.dumps(
                    {
                        "schema_ref": {
                            "contract": "catalog/contracts/execution_schema_revision",
                            "id": "text-to-image",
                            "revision": "1.0.0",
                        },
                        "input_bindings": [
                            {"value": "{{prompt}}", "node_id": "171", "path": ["widgets_values", "0"]}
                        ],
                    }
                ),
                encoding="utf-8",
            )
            source_schema.write_text(
                json.dumps(
                    {
                        "inputs": [{"id": "prompt", "type": "string", "required": True, "max_length": 4000}],
                        "outputs": {"type": "image_set"},
                    }
                ),
                encoding="utf-8",
            )

            extract_runtime_metadata(
                execution_contract_path=source_contract,
                execution_schema_path=source_schema,
                execution_contract_output_path=output,
            )

            value = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual("image_set", value["execution_schema"]["outputs"]["type"])
            self.assertEqual("{{prompt}}", value["input_bindings"][0]["value"])
            self.assertNotIn("schema_ref", value)


if __name__ == "__main__":
    unittest.main()
