import unittest

from runpod_endpoint_worker.schemas import GenerationImage
from runpod_endpoint_worker.service import GenerationService
from helpers import WorkerFixture


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
        with WorkerFixture(executor=executor) as fixture:
            response = fixture.service.generate_from_payload({"execution_type": "t2i", "prompt": "a lamp"})

        payload = response.to_payload()
        self.assertEqual(payload["status"], "succeeded")
        self.assertTrue(payload["generation"]["implemented"])
        self.assertEqual(payload["generation"]["execution_type"], "t2i")
        self.assertEqual(payload["generation"]["images"][0]["artifact_uri"], "runpod-volume://luma-forge/outputs/jobs/job-123/0001/result.png")
        self.assertEqual(payload["generation"]["images"][0]["storage"]["type"], "runpod_volume")
        self.assertNotIn("data_base64", payload["generation"]["images"][0])
        self.assertEqual(executor.request.prompt, "a lamp")

    def test_service_from_config_uses_runtime_executor(self):
        with WorkerFixture() as fixture:
            service = GenerationService.from_config(fixture.config)

        self.assertIsNotNone(service.executor)


if __name__ == "__main__":
    unittest.main()
