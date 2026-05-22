import unittest

from runpod_endpoint_worker.service import GenerationService
from helpers import WorkerFixture


class StubGenerationTests(unittest.TestCase):
    def test_stub_generation_does_not_require_prepared_workspace_files(self):
        with WorkerFixture() as fixture:
            response = fixture.service.generate_from_payload({"execution_type": "t2i", "prompt": "a lamp"})

        payload = response.to_payload()
        self.assertEqual(payload["status"], "succeeded")
        self.assertFalse(payload["generation"]["implemented"])
        self.assertEqual(payload["generation"]["execution_type"], "t2i")
        self.assertIn("not implemented", payload["generation"]["message"])

    def test_service_from_config_uses_stub_only(self):
        with WorkerFixture() as fixture:
            service = GenerationService.from_config(fixture.config)
            response = service.generate_from_payload({"execution_type": "t2i", "prompt": "a lamp"})

        self.assertFalse(response.to_payload()["generation"]["implemented"])


if __name__ == "__main__":
    unittest.main()
