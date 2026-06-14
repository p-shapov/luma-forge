import unittest
from unittest.mock import patch

from runpod_endpoint_worker.errors import ComfyWorkflowError
from runpod_endpoint_worker.handler import create_handler
from runpod_endpoint_worker.schemas import GenerationImage
from helpers import WorkerFixture


class RunPodHandlerTests(unittest.TestCase):
    def test_invalid_input_returns_safe_error(self):
        with WorkerFixture() as fixture:
            handler = create_handler(fixture.service)
            payload = handler({"input": {}})

        self.assertEqual(payload["status"], "failed")
        self.assertEqual(payload["error"], "invalid_request: prompt is required")
        self.assertEqual(payload["failure"]["code"], "invalid_request")
        self.assertEqual(payload["failure"]["stage"], "request_validation")
        self.assertFalse(payload["failure"]["retryable"])
        self.assertIn("message", payload["failure"])

    def test_successful_generation_returns_implemented_response(self):
        class SucceedingExecutor:
            def generate(self, request):
                return [
                    GenerationImage(
                        filename="ComfyUI_00001_.png",
                        mime_type="image/png",
                        byte_size=5,
                        sha256="sha256",
                        artifact_uri="runpod-volume://luma-forge/outputs/jobs/job-123/0001/ComfyUI_00001_.png",
                        storage_type="runpod_volume",
                        relative_path="luma-forge/outputs/jobs/job-123/0001/ComfyUI_00001_.png",
                    )
                ]

        with WorkerFixture(executor=SucceedingExecutor()) as fixture:
            handler = create_handler(fixture.service)
            payload = handler({"id": "job-123", "input": {"prompt": "a lamp"}})

        self.assertEqual(payload["status"], "succeeded")
        self.assertTrue(payload["generation"]["implemented"])
        self.assertEqual(
            payload["generation"]["images"],
            [
                {
                    "filename": "ComfyUI_00001_.png",
                    "mime_type": "image/png",
                    "byte_size": 5,
                    "sha256": "sha256",
                    "artifact_uri": "runpod-volume://luma-forge/outputs/jobs/job-123/0001/ComfyUI_00001_.png",
                    "storage": {
                        "type": "runpod_volume",
                        "relative_path": "luma-forge/outputs/jobs/job-123/0001/ComfyUI_00001_.png",
                    },
                }
            ],
        )

    def test_runtime_error_is_reported_safely(self):
        class FailingService:
            def generate_from_payload(self, payload, **kwargs):
                raise RuntimeError("secret token failed")

        handler = create_handler(FailingService())
        payload = handler({"input": {"prompt": "a lamp"}})

        self.assertEqual(payload["status"], "failed")
        self.assertEqual(payload["error"], "runtime_failed: Endpoint worker runtime failed.")
        self.assertEqual(payload["failure"]["code"], "runtime_failed")
        self.assertEqual(payload["failure"]["message"], "Endpoint worker runtime failed.")
        self.assertEqual(payload["failure"]["stage"], "runtime")
        self.assertTrue(payload["failure"]["retryable"])

    def test_endpoint_worker_error_includes_safe_message(self):
        class FailingService:
            def generate_from_payload(self, payload, **kwargs):
                raise ComfyWorkflowError("ComfyUI workflow execution failed. Missing model file.")

        handler = create_handler(FailingService())
        payload = handler({"input": {"prompt": "a lamp"}})

        self.assertEqual(payload["status"], "failed")
        self.assertEqual(payload["failure"]["code"], "comfyui_workflow_failed")
        self.assertEqual(payload["failure"]["message"], "ComfyUI workflow execution failed. Missing model file.")
        self.assertEqual(payload["failure"]["stage"], "workflow_execution")
        self.assertFalse(payload["failure"]["retryable"])

    def test_endpoint_worker_error_log_uses_safe_context(self):
        class FailingService:
            def generate_from_payload(self, payload, **kwargs):
                raise ComfyWorkflowError("ComfyUI workflow execution failed. password leaked")

        handler = create_handler(FailingService())
        with patch("runpod_endpoint_worker.logging.LOGGER.warning") as warning:
            payload = handler({"id": "job-123", "input": {"prompt": "a lamp"}})

        self.assertEqual(payload["status"], "failed")
        self.assertEqual(payload["failure"]["code"], "comfyui_workflow_failed")
        self.assertEqual(payload["failure"]["message"], "Endpoint worker request failed.")
        self.assertEqual(warning.call_args.args[1], "Endpoint worker request failed")
        self.assertEqual(warning.call_args.args[2], "job-123")
        self.assertEqual(warning.call_args.args[3], "comfyui_workflow_failed")
        self.assertEqual(warning.call_args.args[4], "workflow_execution")
        self.assertFalse(warning.call_args.args[5])
        self.assertEqual(warning.call_args.args[7], "Endpoint worker request failed.")
        self.assertEqual(warning.call_args.args[8], {})

    def test_failure_survives_runpod_sdk_output_normalization(self):
        class FailingService:
            def generate_from_payload(self, payload, **kwargs):
                raise ComfyWorkflowError("ComfyUI workflow execution failed. Missing model file.")

        handler = create_handler(FailingService())
        payload = handler({"input": {"prompt": "a lamp"}})
        normalized = _runpod_sdk_normalize(payload)

        self.assertEqual(normalized["error"], "comfyui_workflow_failed: ComfyUI workflow execution failed. Missing model file.")
        self.assertEqual(normalized["output"]["status"], "failed")
        self.assertEqual(normalized["output"]["failure"]["code"], "comfyui_workflow_failed")
        self.assertEqual(normalized["output"]["failure"]["message"], "ComfyUI workflow execution failed. Missing model file.")
        self.assertEqual(normalized["output"]["failure"]["stage"], "workflow_execution")
        self.assertFalse(normalized["output"]["failure"]["retryable"])

    def test_unexpected_runtime_error_logs_sanitized_original_exception(self):
        class FailingService:
            def generate_from_payload(self, payload, **kwargs):
                raise RuntimeError("secret token failed")

        handler = create_handler(FailingService())
        with patch("runpod_endpoint_worker.logging.LOGGER.warning") as warning:
            handler({"id": "job-123", "input": {"prompt": "a lamp"}})

        self.assertEqual(warning.call_args_list[0].args[1], "Unexpected endpoint worker exception")
        self.assertEqual(warning.call_args_list[0].args[2], "job-123")
        self.assertEqual(warning.call_args_list[0].args[3], "RuntimeError")
        self.assertEqual(warning.call_args_list[0].args[4], "redacted")

    def test_importing_handler_does_not_start_generation(self):
        with WorkerFixture() as fixture:
            create_handler(fixture.service)


def _runpod_sdk_normalize(handler_output):
    job_output = dict(handler_output)
    error = job_output.pop("error", None)
    normalized = {"output": job_output}
    if error:
        normalized["error"] = error
    return normalized


if __name__ == "__main__":
    unittest.main()
