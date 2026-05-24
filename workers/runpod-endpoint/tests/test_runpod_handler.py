import unittest

from runpod_endpoint_worker.handler import create_handler
from runpod_endpoint_worker.schemas import GenerationImage
from helpers import WorkerFixture


class RunPodHandlerTests(unittest.TestCase):
    def test_invalid_input_returns_safe_error(self):
        with WorkerFixture() as fixture:
            handler = create_handler(fixture.service)
            payload = handler({"input": {"execution_type": "i2i", "prompt": "a lamp"}})

        self.assertEqual(payload["status"], "failed")
        self.assertEqual(payload["error"]["code"], "unsupported_execution_type")

    def test_successful_generation_returns_implemented_response(self):
        class SucceedingExecutor:
            def generate(self, request):
                return [
                    GenerationImage(
                        filename="ComfyUI_00001_.png",
                        mime_type="image/png",
                        data_base64="aW1hZ2U=",
                    )
                ]

        with WorkerFixture(executor=SucceedingExecutor()) as fixture:
            handler = create_handler(fixture.service)
            payload = handler({"input": {"execution_type": "t2i", "prompt": "a lamp"}})

        self.assertEqual(payload["status"], "succeeded")
        self.assertTrue(payload["generation"]["implemented"])
        self.assertEqual(payload["generation"]["execution_type"], "t2i")
        self.assertEqual(
            payload["generation"]["images"],
            [
                {
                    "filename": "ComfyUI_00001_.png",
                    "mime_type": "image/png",
                    "data_base64": "aW1hZ2U=",
                }
            ],
        )

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
