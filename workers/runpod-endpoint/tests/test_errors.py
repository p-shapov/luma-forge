import unittest

from runpod_endpoint_worker.errors import ComfyWorkflowError, ValidationError, safe_failure_payload
from runpod_endpoint_worker.logging import _safe_log_message


class ErrorTests(unittest.TestCase):
    def test_safe_failure_payload_redacts_secret_markers(self):
        payload = safe_failure_payload(ValidationError("Bearer token abc failed"))

        self.assertEqual(payload["code"], "invalid_request")
        self.assertEqual(payload["message"], "Endpoint worker request failed.")
        self.assertNotIn("abc", payload["message"])

    def test_safe_failure_payload_preserves_ui_safe_message(self):
        payload = safe_failure_payload(ValidationError("prompt is too large"))

        self.assertEqual(payload["message"], "prompt is too large")

    def test_safe_failure_payload_truncates_long_messages(self):
        payload = safe_failure_payload(ValidationError("x" * 700))

        self.assertEqual(len(payload["message"]), 600)
        self.assertTrue(payload["message"].endswith("..."))

    def test_safe_failure_payload_includes_classification(self):
        payload = safe_failure_payload(ValidationError("prompt is too large"))

        self.assertEqual(payload["stage"], "request_validation")
        self.assertFalse(payload["retryable"])

    def test_safe_failure_payload_allowlists_metadata(self):
        payload = safe_failure_payload(
            ComfyWorkflowError(
                "ComfyUI workflow execution failed.",
                metadata={
                    "exit_status": 1,
                    "stderr": "secret token",
                    "nested": {"unsafe": True},
                },
            )
        )

        self.assertEqual(payload["metadata"], {"exit_status": 1})
        self.assertNotIn("stderr", payload["metadata"])

    def test_safe_log_message_redacts_generated_image_data(self):
        self.assertEqual(_safe_log_message("base64 image data here"), "redacted")

    def test_safe_log_message_uses_payload_secret_markers(self):
        self.assertEqual(_safe_log_message("password leaked"), "redacted")


if __name__ == "__main__":
    unittest.main()
