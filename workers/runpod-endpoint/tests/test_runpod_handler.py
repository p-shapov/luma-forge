import unittest

from runpod_endpoint_worker.handler import create_handler
from helpers import WorkerFixture


class RunPodHandlerTests(unittest.TestCase):
    def test_invalid_input_does_not_return_stub_success(self):
        with WorkerFixture() as fixture:
            handler = create_handler(fixture.service)
            payload = handler({"input": {"execution_type": "i2i", "prompt": "a lamp"}})

        self.assertEqual(payload["status"], "failed")
        self.assertEqual(payload["error"]["code"], "unsupported_execution_type")

    def test_successful_generation_returns_stub_response(self):
        with WorkerFixture() as fixture:
            handler = create_handler(fixture.service)
            payload = handler({"input": {"execution_type": "t2i", "prompt": "a lamp"}})

        self.assertEqual(payload["status"], "succeeded")
        self.assertFalse(payload["generation"]["implemented"])
        self.assertEqual(payload["generation"]["execution_type"], "t2i")

    def test_runtime_error_is_reported_safely(self):
        class FailingService:
            def generate_from_payload(self, payload):
                raise RuntimeError("secret token failed")

        handler = create_handler(FailingService())
        payload = handler({"input": {"execution_type": "t2i", "prompt": "a lamp"}})

        self.assertEqual(payload["status"], "failed")
        self.assertEqual(payload["error"]["code"], "runtime_failed")
        self.assertEqual(payload["error"]["message"], "Endpoint worker runtime failed.")

    def test_importing_handler_does_not_start_generation(self):
        with WorkerFixture() as fixture:
            create_handler(fixture.service)


if __name__ == "__main__":
    unittest.main()
