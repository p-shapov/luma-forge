import tempfile
import time
import unittest
from pathlib import Path

from helpers import BlockingProvisioner, ImmediateProvisioner, ServerFixture, start_payload
from provisioner_worker.config import WorkerConfig
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
        self.assertEqual(payload["active_job_id"], "job-1")

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
        self.assertEqual(payload["error"]["message"], "Git checkout failed.")

    def test_authorized_request_is_accepted_when_token_is_configured(self):
        with ServerFixture(
            ImmediateProvisioner(),
            config=WorkerConfig(bearer_token="secret"),
        ) as server:
            status, payload = server.request("GET", "/status", headers={"Authorization": "Bearer secret"})

        self.assertEqual(status, 200)
        self.assertEqual(payload["status"], "idle")

    def test_unauthorized_request_is_rejected_when_token_is_configured(self):
        with ServerFixture(
            ImmediateProvisioner(),
            config=WorkerConfig(bearer_token="secret"),
        ) as server:
            status, payload = server.request("GET", "/status", headers={"Authorization": "Bearer wrong"})

        self.assertEqual(status, 401)
        self.assertEqual(payload["code"], "unauthorized")
        self.assertNotIn("secret", payload["message"])

    def test_authorization_can_be_disabled(self):
        with ServerFixture(
            ImmediateProvisioner(),
            config=WorkerConfig(bearer_token=None),
        ) as server:
            status, payload = server.request("GET", "/status")

        self.assertEqual(status, 200)
        self.assertEqual(payload["status"], "idle")

    def test_rejects_oversized_request_before_parsing_json(self):
        with ServerFixture(
            ImmediateProvisioner(),
            config=WorkerConfig(max_request_bytes=2),
        ) as server:
            status, payload = server.request("POST", "/start", {"bad": "json"})

        self.assertEqual(status, 413)
        self.assertEqual(payload["code"], "request_too_large")

    def test_rejects_malformed_content_length(self):
        with ServerFixture(ImmediateProvisioner()) as server:
            status, payload = server.raw_request(
                b"POST /start HTTP/1.1\r\n"
                b"Host: 127.0.0.1\r\n"
                b"Content-Type: application/json\r\n"
                b"Content-Length: nope\r\n"
                b"\r\n"
                b"{}",
            )

        self.assertEqual(status, 400)
        self.assertEqual(payload["code"], "invalid_request")


if __name__ == "__main__":
    unittest.main()
