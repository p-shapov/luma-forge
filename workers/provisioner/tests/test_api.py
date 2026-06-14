import tempfile
import time
import unittest
from pathlib import Path

from app.config import ConfigurationError, WorkerConfig
from app.errors import (
    AssetAuthRequiredError,
    AssetDownloadError,
    PathValidationError,
    StepTimeoutError,
)
from helpers import ImmediateProvisioner, RecordingProvisioner, ServerFixture, test_config


class ApiTests(unittest.TestCase):
    def test_auto_start_runs_without_client_input(self):
        with tempfile.TemporaryDirectory() as directory, ServerFixture(
            ImmediateProvisioner(),
            workspace_mount_path=Path(directory),
        ) as server:
            payload = _wait_for_status(server, "succeeded")

        self.assertEqual(payload["status"], "succeeded")
        self.assertEqual(payload["job_id"], "job-1")
        self.assertEqual(payload["progress_percent"], 100)

    def test_running_is_returned_before_job_finishes(self):
        class DelayedProvisioner(ImmediateProvisioner):
            pass

        provisioner = DelayedProvisioner()
        with tempfile.TemporaryDirectory() as directory, ServerFixture(
            provisioner,
            workspace_mount_path=Path(directory),
        ) as server:
            status, payload = server.request("GET", "/status")

            self.assertEqual(status, 200)
            self.assertIn(payload["status"], {"running", "succeeded", "failed"})

    def test_auto_start_fails_on_invalid_start_request_env(self):
        env = {
            "LUMA_FORGE_PROVISIONER_BEARER_TOKEN": "test-token-0123456789abcdef012345",
            "LUMA_FORGE_PROVISIONER_JOB_ID": "job-1",
            "LUMA_FORGE_PROVISIONER_REQUIRES_HUGGING_FACE_API_KEY": "false",
            "LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS": '{"bad": "request"}',
            "LUMA_FORGE_PROVISIONER_HOST": "127.0.0.1",
            "LUMA_FORGE_PROVISIONER_PORT": "8000",
            "LUMA_FORGE_WORKSPACE_MOUNT_PATH": "/workspace",
        }
        with self.assertRaises(ConfigurationError) as context:
            WorkerConfig.from_env(env)

        self.assertEqual(context.exception.env_name, "LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS")
        self.assertEqual(context.exception.code, "invalid_request")

    def test_authorized_request_is_accepted(self):
        config = test_config(bearer_token="authorized-token-0123456789abcdef")
        with ServerFixture(ImmediateProvisioner(), config=config) as server:
            status, payload = server.request("GET", "/status")

        self.assertEqual(status, 200)
        self.assertIn(payload["status"], {"running", "succeeded", "failed"})

    def test_unauthorized_request_is_rejected(self):
        config = test_config(bearer_token="authorized-token-0123456789abcdef")
        with ServerFixture(ImmediateProvisioner(), config=config) as server:
            status, payload = server.request("GET", "/status", headers={"Authorization": "Bearer wrong"})

        self.assertEqual(status, 401)
        self.assertEqual(payload["code"], "invalid_authorization")
        self.assertNotIn("secret", payload["message"])

    def test_unknown_endpoint_returns_not_found_code(self):
        with ServerFixture(ImmediateProvisioner()) as server:
            status, payload = server.request("GET", "/unknown")

        self.assertEqual(status, 404)
        self.assertEqual(payload["code"], "endpoint_not_found")
        self.assertEqual(payload["message"], "Endpoint not found")

    def test_unsupported_method_for_known_path_returns_not_found_when_authorized(self):
        with ServerFixture(ImmediateProvisioner()) as server:
            status, payload = server.raw_request(
                b"PUT /status HTTP/1.1\r\n"
                b"Host: 127.0.0.1\r\n"
                b"Authorization: Bearer test-token-0123456789abcdef012345\r\n"
                b"\r\n",
            )

        self.assertEqual(status, 404)
        self.assertEqual(payload["code"], "endpoint_not_found")

    def test_unknown_post_endpoint_does_not_parse_body(self):
        with ServerFixture(ImmediateProvisioner()) as server:
            status, payload = server.raw_request(
                b"POST /unknown HTTP/1.1\r\n"
                b"Host: 127.0.0.1\r\n"
                b"Authorization: Bearer test-token-0123456789abcdef012345\r\n"
                b"Content-Type: application/json\r\n"
                b"Content-Length: 1\r\n"
                b"\r\n"
                b"{",
            )

        self.assertEqual(status, 404)
        self.assertEqual(payload["code"], "endpoint_not_found")

    def test_failed_job_reports_expected_error_codes(self):
        cases = [
            (AssetDownloadError("Asset download failed."), "asset_download_failed"),
            (AssetAuthRequiredError("Asset auth required."), "asset_auth_required"),
            (PathValidationError("path must be safe"), "path_validation_failed"),
            (StepTimeoutError("Provisioning step timed out."), "step_timeout"),
        ]

        for error, expected_code in cases:
            with self.subTest(error=error.__class__.__name__):
                provisioner = RecordingProvisioner(error)
                with tempfile.TemporaryDirectory() as directory, ServerFixture(
                    provisioner,
                    workspace_mount_path=Path(directory),
                ) as server:
                    payload = _wait_for_status(server, "failed")

                self.assertTrue(provisioner.called)
                self.assertEqual(payload["status"], "failed")
                self.assertEqual(payload["error"]["code"], expected_code)
                self.assertEqual(payload["error"]["message"], error.message)


def _wait_for_status(server: ServerFixture, status: str):
    payload = {}
    for _ in range(80):
        _, payload = server.request("GET", "/status")
        if payload["status"] == status:
            break
        time.sleep(0.02)
    return payload


if __name__ == "__main__":
    unittest.main()
