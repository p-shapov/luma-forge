import tempfile
import time
import unittest
from pathlib import Path

from helpers import BlockingProvisioner, ImmediateProvisioner, ServerFixture, start_payload, test_config
from provisioner_worker.errors import GitCheckoutError


class ApiTests(unittest.TestCase):
    def test_idle_status(self):
        with ServerFixture(ImmediateProvisioner()) as server:
            status, payload = server.request("GET", "/status")

        self.assertEqual(status, 200)
        self.assertEqual(payload["status"], "idle")
        self.assertIsNone(payload["job_id"])

    def test_start_accepts_valid_request(self):
        with tempfile.TemporaryDirectory() as directory, ServerFixture(
            ImmediateProvisioner(),
            workspace_mount_path=Path(directory),
        ) as server:
            status, payload = server.request("POST", "/start", start_payload(Path(directory)))

        self.assertEqual(status, 202)
        self.assertEqual(payload["status"], "running")
        self.assertEqual(payload["job_id"], "job-1")

    def test_start_rejects_invalid_request(self):
        with ServerFixture(ImmediateProvisioner()) as server:
            status, payload = server.request("POST", "/start", {"job_id": ""})

        self.assertEqual(status, 400)
        self.assertEqual(payload["code"], "invalid_request")
        self.assertEqual(payload["reason_code"], "invalid_request")

    def test_start_conflicts_while_job_is_active(self):
        provisioner = BlockingProvisioner()
        with tempfile.TemporaryDirectory() as directory, ServerFixture(
            provisioner,
            workspace_mount_path=Path(directory),
        ) as server:
            first_status, _ = server.request("POST", "/start", start_payload(Path(directory)))
            self.assertTrue(provisioner.started.wait(2))
            second_status, payload = server.request("POST", "/start", start_payload(Path(directory), job_id="job-2"))
            provisioner.release.set()

        self.assertEqual(first_status, 202)
        self.assertEqual(second_status, 409)
        self.assertEqual(payload["code"], "job_already_running")
        self.assertEqual(payload["reason_code"], "active_job_exists")
        self.assertEqual(payload["context"], {"active_job_id": "job-1"})

    def test_cancel_active_job(self):
        provisioner = BlockingProvisioner()
        with tempfile.TemporaryDirectory() as directory, ServerFixture(
            provisioner,
            workspace_mount_path=Path(directory),
        ) as server:
            server.request("POST", "/start", start_payload(Path(directory)))
            self.assertTrue(provisioner.started.wait(2))
            status, payload = server.request("POST", "/cancel", {"job_id": "job-1"})
            for _ in range(50):
                _, latest = server.request("GET", "/status")
                if latest["status"] == "cancelled":
                    break
                time.sleep(0.02)

        self.assertEqual(status, 202)
        self.assertEqual(payload["status"], "cancelling")
        self.assertEqual(latest["status"], "cancelled")

    def test_cancel_unmatched_job_is_rejected(self):
        with ServerFixture(ImmediateProvisioner()) as server:
            status, payload = server.request("POST", "/cancel", {"job_id": "unknown"})

        self.assertEqual(status, 400)
        self.assertEqual(payload["code"], "invalid_request")
        self.assertEqual(payload["reason_code"], "no_matching_active_job")

    def test_success_status_after_job_finishes(self):
        with tempfile.TemporaryDirectory() as directory, ServerFixture(
            ImmediateProvisioner(),
            workspace_mount_path=Path(directory),
        ) as server:
            server.request("POST", "/start", start_payload(Path(directory)))
            for _ in range(50):
                _, payload = server.request("GET", "/status")
                if payload["status"] == "succeeded":
                    break
                time.sleep(0.02)

        self.assertEqual(payload["status"], "succeeded")
        self.assertEqual(payload["progress_percent"], 100)

    def test_start_rejects_unconfigured_workspace_mount_path(self):
        with tempfile.TemporaryDirectory() as allowed, tempfile.TemporaryDirectory() as other, ServerFixture(
            ImmediateProvisioner(),
            workspace_mount_path=Path(allowed),
        ) as server:
            status, payload = server.request("POST", "/start", start_payload(Path(other)))

        self.assertEqual(status, 400)
        self.assertEqual(payload["code"], "invalid_request")
        self.assertEqual(payload["reason_code"], "workspace_mount_path_mismatch")
        self.assertEqual(payload["context"], {"field": "workspace_mount_path"})

    def test_failed_job_reports_specific_error_code(self):
        class FailingProvisioner(ImmediateProvisioner):
            def prepare(self, request, progress, cancel_event):
                raise GitCheckoutError("Git checkout failed.")

        with tempfile.TemporaryDirectory() as directory, ServerFixture(
            FailingProvisioner(),
            workspace_mount_path=Path(directory),
        ) as server:
            server.request("POST", "/start", start_payload(Path(directory)))
            for _ in range(50):
                _, payload = server.request("GET", "/status")
                if payload["status"] == "failed":
                    break
                time.sleep(0.02)

        self.assertEqual(payload["status"], "failed")
        self.assertEqual(payload["error"]["code"], "git_checkout_failed")
        self.assertEqual(payload["error"]["reason_code"], "git_checkout_failed")
        self.assertEqual(payload["error"]["message"], "Git checkout failed.")

    def test_authorized_request_is_accepted(self):
        config = test_config(bearer_token="authorized-token-0123456789abcdef")
        with ServerFixture(ImmediateProvisioner(), config=config) as server:
            status, payload = server.request("GET", "/status")

        self.assertEqual(status, 200)
        self.assertEqual(payload["status"], "idle")

    def test_unauthorized_request_is_rejected(self):
        config = test_config(bearer_token="authorized-token-0123456789abcdef")
        with ServerFixture(ImmediateProvisioner(), config=config) as server:
            status, payload = server.request("GET", "/status", headers={"Authorization": "Bearer wrong"})

        self.assertEqual(status, 401)
        self.assertEqual(payload["code"], "unauthorized")
        self.assertEqual(payload["reason_code"], "invalid_authorization")
        self.assertNotIn("secret", payload["message"])

    def test_missing_authorization_is_rejected(self):
        with ServerFixture(ImmediateProvisioner()) as server:
            status, payload = server.request("GET", "/status", authorize=False)

        self.assertEqual(status, 401)
        self.assertEqual(payload["code"], "unauthorized")
        self.assertEqual(payload["reason_code"], "invalid_authorization")

    def test_rejects_oversized_request_before_parsing_json(self):
        with ServerFixture(
            ImmediateProvisioner(),
            config=test_config(max_request_bytes=2),
        ) as server:
            status, payload = server.request("POST", "/start", {"bad": "json"})

        self.assertEqual(status, 413)
        self.assertEqual(payload["code"], "request_too_large")
        self.assertEqual(payload["reason_code"], "request_body_too_large")
        self.assertEqual(payload["context"], {"max_request_bytes": 2})

    def test_rejects_malformed_content_length(self):
        with ServerFixture(ImmediateProvisioner()) as server:
            status, payload = server.raw_request(
                b"POST /start HTTP/1.1\r\n"
                b"Host: 127.0.0.1\r\n"
                b"Authorization: Bearer test-token-0123456789abcdef012345\r\n"
                b"Content-Type: application/json\r\n"
                b"Content-Length: nope\r\n"
                b"\r\n"
                b"{}",
            )

        self.assertEqual(status, 400)
        self.assertEqual(payload["code"], "invalid_request")
        self.assertEqual(payload["reason_code"], "malformed_content_length")

    def test_unknown_endpoint_includes_reason_code(self):
        with ServerFixture(ImmediateProvisioner()) as server:
            status, payload = server.request("GET", "/unknown")

        self.assertEqual(status, 404)
        self.assertEqual(payload["code"], "not_found")
        self.assertEqual(payload["reason_code"], "endpoint_not_found")

    def test_invalid_json_includes_reason_code(self):
        with ServerFixture(ImmediateProvisioner()) as server:
            status, payload = server.raw_request(
                b"POST /start HTTP/1.1\r\n"
                b"Host: 127.0.0.1\r\n"
                b"Authorization: Bearer test-token-0123456789abcdef012345\r\n"
                b"Content-Type: application/json\r\n"
                b"Content-Length: 1\r\n"
                b"\r\n"
                b"{",
            )

        self.assertEqual(status, 400)
        self.assertEqual(payload["code"], "invalid_json")
        self.assertEqual(payload["reason_code"], "invalid_json")

    def test_worker_error_payload_does_not_include_authorization_value(self):
        token = "authorized-token-0123456789abcdef"
        config = test_config(bearer_token=token)
        with ServerFixture(ImmediateProvisioner(), config=config) as server:
            _, payload = server.request("GET", "/status", headers={"Authorization": "Bearer wrong"})

        self.assertNotIn(token, str(payload))
        self.assertNotIn("wrong", str(payload))


if __name__ == "__main__":
    unittest.main()
