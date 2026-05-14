import unittest

from runpod_endpoint_worker.errors import ComfyUiExecutionError
from runpod_endpoint_worker.handler import create_handler
from helpers import FakeComfyUiClient, WorkerFixture


class RunPodHandlerTests(unittest.TestCase):
    def test_invalid_input_does_not_call_comfyui(self):
        with WorkerFixture() as fixture:
            handler = create_handler(fixture.service)
            payload = handler({"input": {"execution_type": "i2i", "prompt": "a lamp"}})

        self.assertEqual(payload["status"], "failed")
        self.assertEqual(payload["error"]["code"], "unsupported_execution_type")
        self.assertEqual(fixture.comfyui.queued_workflows, [])

    def test_successful_generation_returns_single_image(self):
        with WorkerFixture() as fixture:
            handler = create_handler(fixture.service)
            payload = handler({"input": {"execution_type": "t2i", "prompt": "a lamp"}})

        self.assertEqual(payload["status"], "succeeded")
        self.assertEqual(payload["image"]["mime_type"], "image/png")
        self.assertIn("data", payload["image"])

    def test_runtime_error_is_reported_safely(self):
        with WorkerFixture(comfyui=FakeComfyUiClient(fail_on_queue=ComfyUiExecutionError("secret token failed"))) as fixture:
            handler = create_handler(fixture.service)
            payload = handler({"input": {"execution_type": "t2i", "prompt": "a lamp"}})

        self.assertEqual(payload["status"], "failed")
        self.assertEqual(payload["error"]["code"], "comfyui_execution_failed")
        self.assertEqual(payload["error"]["message"], "Endpoint worker request failed.")

    def test_importing_handler_does_not_start_generation(self):
        with WorkerFixture() as fixture:
            create_handler(fixture.service)

        self.assertEqual(fixture.comfyui.queued_workflows, [])


if __name__ == "__main__":
    unittest.main()
