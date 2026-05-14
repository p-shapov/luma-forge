import unittest

from runpod_endpoint_worker.errors import ValidationError, safe_error_payload
from runpod_endpoint_worker.logging import _safe_log_message


class ErrorTests(unittest.TestCase):
    def test_safe_error_payload_redacts_secret_markers(self):
        payload = safe_error_payload(ValidationError("Bearer token abc failed"))

        self.assertEqual(payload["code"], "invalid_request")
        self.assertEqual(payload["message"], "Endpoint worker request failed.")
        self.assertNotIn("abc", payload["message"])

    def test_safe_error_payload_preserves_ui_safe_message(self):
        payload = safe_error_payload(ValidationError("prompt is too large"))

        self.assertEqual(payload["message"], "prompt is too large")

    def test_safe_log_message_redacts_generated_image_data(self):
        self.assertEqual(_safe_log_message("base64 image data here"), "redacted")


if __name__ == "__main__":
    unittest.main()
