import json
import unittest

from app.errors import (
    AssetDownloadError,
    ConflictError,
    RequestTooLargeError,
    UnauthorizedError,
    ValidationError,
)


class WorkerErrorTests(unittest.TestCase):
    def test_serializes_code_reason_code_and_message(self):
        error = UnauthorizedError("Unauthorized.")

        payload = error.to_dict()

        self.assertEqual(payload["code"], "unauthorized")
        self.assertEqual(payload["reason_code"], "invalid_authorization")
        self.assertEqual(payload["message"], "Unauthorized.")
        self.assertNotIn("context", payload)

    def test_serializes_safe_context_when_present(self):
        error = ConflictError(
            "Provisioner worker already has an active job.",
            context={"active_job_id": "job-1"},
        )

        payload = error.to_dict()

        self.assertEqual(payload["code"], "job_already_running")
        self.assertEqual(payload["reason_code"], "active_job_exists")
        self.assertEqual(payload["context"], {"active_job_id": "job-1"})

    def test_allows_reason_code_override_for_specific_validation_failures(self):
        error = ValidationError(
            "Content-Length header must be an integer.",
            reason_code="malformed_content_length",
        )

        self.assertEqual(error.to_dict()["reason_code"], "malformed_content_length")

    def test_preparation_errors_use_stable_reason_codes(self):
        payload = AssetDownloadError("Asset download failed.").to_dict()

        self.assertEqual(payload["code"], "asset_download_failed")
        self.assertEqual(payload["reason_code"], "asset_download_failed")

    def test_serialized_errors_do_not_include_unpassed_unsafe_values(self):
        secret = "Bearer secret-token-0123456789abcdef"
        payload = RequestTooLargeError(
            "Request body is too large.",
            context={"max_request_bytes": 2},
        ).to_dict()

        self.assertNotIn(secret, json.dumps(payload))
        self.assertEqual(payload["context"], {"max_request_bytes": 2})


if __name__ == "__main__":
    unittest.main()
