import json
import sys
import tempfile
import unittest
from pathlib import Path

from app.config import EndpointConfig
from app.schemas import GenerationImage
from app.service import GenerationService

sys.path.insert(0, str(Path(__file__).resolve().parent))
from helpers import WorkerFixture


def _write_contract(directory: str) -> Path:
    path = Path(directory) / "execution-contract.json"
    path.write_text(
        json.dumps(
            {
                "execution_schema": {
                    "version": "1.0.0",
                    "inputs": [
                        {
                            "id": "prompt",
                            "type": "string",
                            "required": True,
                            "max_length": 4000,
                        }
                    ],
                    "outputs": {
                        "type": "image_set",
                    },
                },
                "input_bindings": [],
            }
        ),
        encoding="utf-8",
    )
    return path


class GenerationServiceTests(unittest.TestCase):
    def test_generation_uses_executor_and_returns_artifact_images(self):
        class Executor:
            def generate(self, request):
                self.request = request
                return [
                    GenerationImage(
                        filename="result.png",
                        mime_type="image/png",
                        byte_size=6,
                        sha256="sha256",
                        artifact_uri="runpod-volume://luma-forge/outputs/jobs/job-123/0001/result.png",
                        storage_type="runpod_volume",
                        relative_path="luma-forge/outputs/jobs/job-123/0001/result.png",
                    )
                ]

        executor = Executor()
        with tempfile.TemporaryDirectory() as directory:
            config = EndpointConfig(execution_contract_path=_write_contract(directory))
            with WorkerFixture(config=config, executor=executor) as fixture:
                response = fixture.service.generate_from_payload({"prompt": "a lamp"})

        payload = response.to_payload()
        self.assertEqual(payload["status"], "succeeded")
        self.assertTrue(payload["generation"]["implemented"])
        self.assertEqual(payload["generation"]["images"][0]["artifact_uri"], "runpod-volume://luma-forge/outputs/jobs/job-123/0001/result.png")
        self.assertEqual(payload["generation"]["images"][0]["storage"]["type"], "runpod_volume")
        self.assertNotIn("data_base64", payload["generation"]["images"][0])
        self.assertEqual(executor.request.inputs["prompt"], "a lamp")

    def test_service_from_config_uses_runtime_executor(self):
        with tempfile.TemporaryDirectory() as directory:
            config = EndpointConfig(execution_contract_path=_write_contract(directory))
            with WorkerFixture(config=config) as fixture:
                service = GenerationService.from_config(fixture.config)

        self.assertIsNotNone(service.executor)


if __name__ == "__main__":
    unittest.main()
