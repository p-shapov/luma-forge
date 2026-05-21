from contextlib import redirect_stderr, redirect_stdout
from copy import deepcopy
from io import StringIO
import sys
import tempfile
import time
import unittest
from pathlib import Path
from threading import Event

from helpers import BlockingProvisioner, ImmediateProvisioner, RecordingProvisioner, ServerFixture, start_payload, test_config
from app.errors import (
    AssetAuthRequiredError,
    AssetDownloadError,
    DependencyInstallError,
    GitCheckoutError,
    PathValidationError,
    StepTimeoutError,
)


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
            status, payload = server.request("POST", "/start", start_payload())

        self.assertEqual(status, 202)
        self.assertEqual(payload["status"], "running")
        self.assertEqual(payload["job_id"], "job-1")

    def test_start_rejects_invalid_request(self):
        with ServerFixture(ImmediateProvisioner()) as server:
            status, payload = server.request("POST", "/start", {"job_id": ""})

        self.assertEqual(status, 400)
        self.assertEqual(payload["code"], "invalid_request")
        self.assertEqual(payload["reason_code"], "invalid_request")

    def test_invalid_start_has_no_side_effects(self):
        provisioner = RecordingProvisioner()
        with tempfile.TemporaryDirectory() as directory, ServerFixture(
            provisioner,
            workspace_mount_path=Path(directory),
        ) as server:
            status, payload = server.request("POST", "/start", {"job_id": ""})
            _, latest = server.request("GET", "/status")
            workspace_entries = list(Path(directory).iterdir())

        self.assertEqual(status, 400)
        self.assertEqual(payload["code"], "invalid_request")
        self.assertEqual(latest["status"], "idle")
        self.assertFalse(provisioner.called)
        self.assertEqual(workspace_entries, [])

    def test_unsafe_start_payloads_have_no_side_effects(self):
        def set_custom_node_path(payload, value):
            payload["workflow_preset"]["required_custom_nodes"] = [_custom_node(value)]

        def set_model_path(payload, value):
            payload["workflow_preset"]["required_model_assets"][0]["install"]["comfyui_relative_path"] = value

        cases = [
            ("unsafe workflow id", lambda payload: payload["workflow_preset"].update({"id": "../unsafe"})),
            ("unsafe custom node path", lambda payload: set_custom_node_path(payload, "models/node")),
            ("unsafe model path", lambda payload: set_model_path(payload, "../model.safetensors")),
            (
                "mutable runtime image",
                lambda payload: payload["resolved_runtime_image"].update(
                    {"provisioner_image_ref": "ghcr.io/luma-forge/provisioner-worker:latest"}
                ),
            ),
        ]

        for name, mutate in cases:
            with self.subTest(name=name):
                payload = start_payload()
                mutate(payload)
                provisioner = RecordingProvisioner()
                with tempfile.TemporaryDirectory() as directory, ServerFixture(
                    provisioner,
                    workspace_mount_path=Path(directory),
                ) as server:
                    status, response = server.request("POST", "/start", payload)
                    _, latest = server.request("GET", "/status")
                    workspace_entries = list(Path(directory).iterdir())

                self.assertEqual(status, 400)
                self.assertIn(response["code"], {"invalid_request", "path_validation_failed"})
                self.assertEqual(latest["status"], "idle")
                self.assertFalse(provisioner.called)
                self.assertEqual(workspace_entries, [])

    def test_start_conflicts_while_job_is_active(self):
        provisioner = BlockingProvisioner()
        with tempfile.TemporaryDirectory() as directory, ServerFixture(
            provisioner,
            workspace_mount_path=Path(directory),
        ) as server:
            first_status, _ = server.request("POST", "/start", start_payload())
            self.assertTrue(provisioner.started.wait(2))
            second_status, payload = server.request("POST", "/start", start_payload(job_id="job-2"))
            provisioner.release.set()

        self.assertEqual(first_status, 202)
        self.assertEqual(second_status, 409)
        self.assertEqual(payload["code"], "job_already_running")
        self.assertEqual(payload["reason_code"], "active_job_exists")
        self.assertEqual(payload["context"], {"active_job_id": "job-1"})

    def test_cancel_endpoint_is_not_available(self):
        with ServerFixture(ImmediateProvisioner()) as server:
            status, payload = server.request("POST", "/cancel", {"job_id": "job-1"})

        self.assertEqual(status, 404)
        self.assertEqual(payload["code"], "not_found")
        self.assertEqual(payload["reason_code"], "endpoint_not_found")

    def test_success_status_after_job_finishes(self):
        with tempfile.TemporaryDirectory() as directory, ServerFixture(
            ImmediateProvisioner(),
            workspace_mount_path=Path(directory),
        ) as server:
            server.request("POST", "/start", start_payload())
            for _ in range(50):
                _, payload = server.request("GET", "/status")
                if payload["status"] == "succeeded":
                    break
                time.sleep(0.02)

        self.assertEqual(payload["status"], "succeeded")
        self.assertEqual(payload["progress_percent"], 100)

    def test_failed_job_reports_expected_error_codes(self):
        cases = [
            (GitCheckoutError("Git checkout failed."), "git_checkout_failed", "git_checkout_failed"),
            (
                DependencyInstallError("Dependency installation failed."),
                "dependency_install_failed",
                "dependency_install_failed",
            ),
            (AssetDownloadError("Asset download failed."), "asset_download_failed", "asset_download_failed"),
            (AssetAuthRequiredError("Asset auth required."), "asset_auth_required", "asset_auth_required"),
            (PathValidationError("path must be safe"), "path_validation_failed", "path_validation_failed"),
            (StepTimeoutError("Provisioning step timed out."), "step_timeout", "step_timeout"),
        ]

        for error, expected_code, expected_reason in cases:
            with self.subTest(error=error.__class__.__name__):
                provisioner = RecordingProvisioner(error)
                with tempfile.TemporaryDirectory() as directory, ServerFixture(
                    provisioner,
                    workspace_mount_path=Path(directory),
                ) as server:
                    server.request("POST", "/start", start_payload())
                    payload = _wait_for_status(server, "failed")

                self.assertTrue(provisioner.called)
                self.assertEqual(payload["status"], "failed")
                self.assertEqual(payload["error"]["code"], expected_code)
                self.assertEqual(payload["error"]["reason_code"], expected_reason)
                self.assertEqual(payload["error"]["message"], error.message)

    def test_running_status_does_not_include_console_output(self):
        raw_output = "raw-pip-output-with-credential-url"
        provisioner = ConsoleOutputProvisioner(raw_output)
        stdout = StringIO()
        stderr = StringIO()

        with redirect_stdout(stdout), redirect_stderr(stderr):
            with tempfile.TemporaryDirectory() as directory, ServerFixture(
                provisioner,
                workspace_mount_path=Path(directory),
            ) as server:
                server.request("POST", "/start", start_payload())
                self.assertTrue(provisioner.started.wait(2))
                _, payload = server.request("GET", "/status")
                provisioner.release.set()

        self.assertIn(raw_output, stdout.getvalue())
        self.assertIn(raw_output, stderr.getvalue())
        self.assertEqual(payload["status"], "running")
        self.assertEqual(payload["phase"], "materializing_runtime")
        self.assertEqual(payload["progress_percent"], 25)
        self.assertNotIn(raw_output, str(payload))

    def test_failed_status_does_not_include_console_output(self):
        raw_output = "raw-pip-failure-output-with-credential-url"
        provisioner = ConsoleOutputProvisioner(
            raw_output,
            error=DependencyInstallError("Command failed: python -m"),
        )
        stdout = StringIO()
        stderr = StringIO()

        with redirect_stdout(stdout), redirect_stderr(stderr):
            with tempfile.TemporaryDirectory() as directory, ServerFixture(
                provisioner,
                workspace_mount_path=Path(directory),
            ) as server:
                server.request("POST", "/start", start_payload())
                payload = _wait_for_status(server, "failed")

        self.assertIn(raw_output, stdout.getvalue())
        self.assertIn(raw_output, stderr.getvalue())
        self.assertEqual(payload["status"], "failed")
        self.assertEqual(payload["error"]["code"], "dependency_install_failed")
        self.assertEqual(payload["error"]["reason_code"], "dependency_install_failed")
        self.assertNotIn(raw_output, str(payload))

    def test_unexpected_job_error_is_sanitized(self):
        secret = "secret-token-0123456789abcdef"

        class UnexpectedProvisioner(ImmediateProvisioner):
            def prepare(self, request, progress, cancel_event):
                raise RuntimeError(f"unexpected failure {secret}")

        stderr = StringIO()
        with redirect_stderr(stderr):
            with tempfile.TemporaryDirectory() as directory, ServerFixture(
                UnexpectedProvisioner(),
                workspace_mount_path=Path(directory),
            ) as server:
                server.request("POST", "/start", start_payload())
                payload = _wait_for_status(server, "failed")

        self.assertEqual(payload["status"], "failed")
        self.assertEqual(payload["error"]["code"], "unexpected_error")
        self.assertEqual(payload["error"]["reason_code"], "unexpected_exception")
        self.assertNotIn(secret, str(payload))
        self.assertNotIn(secret, stderr.getvalue())
        self.assertNotIn("Traceback", stderr.getvalue())

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

    def test_non_ascii_authorization_is_rejected(self):
        config = test_config(bearer_token="authorized-token-0123456789abcdef")
        with ServerFixture(ImmediateProvisioner(), config=config) as server:
            status, payload = server.request("GET", "/status", headers={"Authorization": "Bearer é"})

        self.assertEqual(status, 401)
        self.assertEqual(payload["code"], "unauthorized")
        self.assertEqual(payload["reason_code"], "invalid_authorization")

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

    def test_unknown_post_endpoint_does_not_parse_malformed_json(self):
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
        self.assertEqual(payload["code"], "not_found")
        self.assertEqual(payload["reason_code"], "endpoint_not_found")

    def test_unknown_post_endpoint_does_not_enforce_body_size(self):
        with ServerFixture(
            ImmediateProvisioner(),
            config=test_config(max_request_bytes=2),
        ) as server:
            status, payload = server.raw_request(
                b"POST /unknown HTTP/1.1\r\n"
                b"Host: 127.0.0.1\r\n"
                b"Authorization: Bearer test-token-0123456789abcdef012345\r\n"
                b"Content-Type: application/json\r\n"
                b"Content-Length: 3\r\n"
                b"\r\n"
                b"xxx",
            )

        self.assertEqual(status, 404)
        self.assertEqual(payload["code"], "not_found")
        self.assertEqual(payload["reason_code"], "endpoint_not_found")

    def test_unsupported_method_requires_authorization(self):
        with ServerFixture(ImmediateProvisioner()) as server:
            status, payload = server.raw_request(
                b"PUT /status HTTP/1.1\r\n"
                b"Host: 127.0.0.1\r\n"
                b"\r\n",
            )

        self.assertEqual(status, 401)
        self.assertEqual(payload["code"], "unauthorized")
        self.assertEqual(payload["reason_code"], "invalid_authorization")

    def test_unsupported_method_rejects_invalid_authorization(self):
        with ServerFixture(ImmediateProvisioner()) as server:
            status, payload = server.raw_request(
                b"PUT /status HTTP/1.1\r\n"
                b"Host: 127.0.0.1\r\n"
                b"Authorization: Bearer wrong\r\n"
                b"\r\n",
            )

        self.assertEqual(status, 401)
        self.assertEqual(payload["code"], "unauthorized")
        self.assertEqual(payload["reason_code"], "invalid_authorization")
        self.assertNotIn("wrong", str(payload))

    def test_unsupported_method_returns_json_not_found_when_authorized(self):
        with ServerFixture(ImmediateProvisioner()) as server:
            status, payload = server.raw_request(
                b"PUT /status HTTP/1.1\r\n"
                b"Host: 127.0.0.1\r\n"
                b"Authorization: Bearer test-token-0123456789abcdef012345\r\n"
                b"\r\n",
            )

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


def _wait_for_status(server: ServerFixture, status: str) -> dict:
    for _ in range(50):
        _, payload = server.request("GET", "/status")
        if payload["status"] == status:
            return payload
        time.sleep(0.02)
    return payload


class ConsoleOutputProvisioner:
    def __init__(self, raw_output: str, error: Exception | None = None):
        self.raw_output = raw_output
        self.error = error
        self.started = Event()
        self.release = Event()

    def prepare(self, request, progress, cancel_event):
        progress(
            "materializing_runtime",
            25,
            "Validating image-baked ComfyUI runtime",
        )
        print(self.raw_output, flush=True)
        print(self.raw_output, file=sys.stderr, flush=True)
        self.started.set()
        if self.error is not None:
            raise self.error
        while not self.release.is_set() and not cancel_event.is_set():
            self.release.wait(0.01)


def _custom_node(comfyui_custom_nodes_relative_path: str) -> dict:
    payload = deepcopy(start_payload()["workflow_preset"])
    node = {
        "id": "example-node",
        "name": "Example Node",
        "git_source": {
            "source_type": "git",
            "repository_url": "https://example.test/node.git",
            "revision": "0123456789abcdef0123456789abcdef01234567",
        },
        "install": {
            "comfyui_custom_nodes_relative_path": comfyui_custom_nodes_relative_path,
        },
    }
    return node


if __name__ == "__main__":
    unittest.main()
