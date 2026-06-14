import json
import tempfile
import unittest
from pathlib import Path

from workers.runpod_endpoint_build_metadata import extract_runtime_metadata


class BuildMetadataTests(unittest.TestCase):
    def test_extracts_execution_contract_and_schema_revision(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            workflow_catalog = root / "workflow-catalog.json"
            execution_schemas = root / "execution-schemas.json"
            contract_output = root / "execution-contract.json"
            schema_output = root / "execution-schema.json"
            workflow_catalog.write_text(
                json.dumps(
                    {
                        "workflow_presets": [
                            {
                                "id": "comfyui-hidream-o1-dev",
                                "revisions": [
                                    {
                                        "version": "1.0.0",
                                        "execution_contract": {
                                            "schema_ref": {"id": "text-to-image", "version": "1.0.0"},
                                            "input_bindings": [
                                                {"value": "{{prompt}}", "node_id": "171", "path": ["widgets_values", "0"]}
                                            ],
                                        },
                                    }
                                ],
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )
            execution_schemas.write_text(
                json.dumps(
                    {
                        "execution_schemas": [
                            {
                                "id": "text-to-image",
                                "revisions": [
                                    {
                                        "version": "1.0.0",
                                        "inputs": [
                                            {
                                                "id": "prompt",
                                                "type": "string",
                                                "required": True,
                                                "max_length": 4000,
                                            }
                                        ],
                                        "outputs": {"type": "image_set"},
                                    }
                                ],
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )

            extract_runtime_metadata(
                workflow_catalog_path=workflow_catalog,
                execution_schemas_path=execution_schemas,
                workflow_id="comfyui-hidream-o1-dev",
                workflow_version="1.0.0",
                execution_contract_output_path=contract_output,
                execution_schema_output_path=schema_output,
            )

            self.assertEqual("text-to-image", json.loads(contract_output.read_text(encoding="utf-8"))["schema_ref"]["id"])
            self.assertEqual("image_set", json.loads(schema_output.read_text(encoding="utf-8"))["outputs"]["type"])


if __name__ == "__main__":
    unittest.main()
