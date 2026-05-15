from contextlib import redirect_stderr
from io import StringIO
import json
import socket
import unittest
from unittest.mock import patch
from pathlib import Path

from app.config import (
    ConfigurationError,
    DEFAULT_DEPENDENCY_TIMEOUT_SECONDS,
    DEFAULT_DOWNLOAD_TIMEOUT_SECONDS,
    DEFAULT_GIT_TIMEOUT_SECONDS,
    DEFAULT_HOST,
    DEFAULT_MAX_REQUEST_BYTES,
    DEFAULT_PORT,
    DEFAULT_WORKSPACE_MOUNT_PATH,
    MAX_REQUEST_BYTES_LIMIT,
    MAX_TIMEOUT_SECONDS,
    WorkerConfig,
)
from app.server import create_server, main


VALID_TOKEN = "config-token-0123456789abcdef0123"


def valid_env(**overrides):
    env = {
        "LUMA_FORGE_PROVISIONER_BEARER_TOKEN": VALID_TOKEN,
    }
    env.update(overrides)
    return env


class ConfigTests(unittest.TestCase):
    def test_valid_config_uses_defaults_for_optional_values(self):
        config = WorkerConfig.from_env(valid_env())

        self.assertEqual(config.host, DEFAULT_HOST)
        self.assertEqual(config.port, DEFAULT_PORT)
        self.assertEqual(config.bearer_token, VALID_TOKEN)
        self.assertEqual(config.max_request_bytes, DEFAULT_MAX_REQUEST_BYTES)
        self.assertEqual(config.git_timeout_seconds, DEFAULT_GIT_TIMEOUT_SECONDS)
        self.assertEqual(config.dependency_timeout_seconds, DEFAULT_DEPENDENCY_TIMEOUT_SECONDS)
        self.assertEqual(config.download_timeout_seconds, DEFAULT_DOWNLOAD_TIMEOUT_SECONDS)
        self.assertEqual(config.workspace_mount_path, Path(DEFAULT_WORKSPACE_MOUNT_PATH).resolve(strict=False))

    def test_valid_config_accepts_explicit_values(self):
        config = WorkerConfig.from_env(
            valid_env(
                LUMA_FORGE_PROVISIONER_HOST="worker.internal",
                LUMA_FORGE_PROVISIONER_PORT="9000",
                LUMA_FORGE_PROVISIONER_MAX_REQUEST_BYTES="2048",
                LUMA_FORGE_PROVISIONER_GIT_TIMEOUT_SECONDS="12.5",
                LUMA_FORGE_PROVISIONER_DEPENDENCY_TIMEOUT_SECONDS="13.5",
                LUMA_FORGE_PROVISIONER_DOWNLOAD_TIMEOUT_SECONDS="14.5",
                LUMA_FORGE_WORKSPACE_MOUNT_PATH="/workspace/custom",
            )
        )

        self.assertEqual(config.host, "worker.internal")
        self.assertEqual(config.port, 9000)
        self.assertEqual(config.max_request_bytes, 2048)
        self.assertEqual(config.git_timeout_seconds, 12.5)
        self.assertEqual(config.dependency_timeout_seconds, 13.5)
        self.assertEqual(config.download_timeout_seconds, 14.5)
        self.assertEqual(config.workspace_mount_path, Path("/workspace/custom").resolve(strict=False))

    def test_rejects_missing_bearer_token_without_leaking_value(self):
        with self.assertRaises(ConfigurationError) as context:
            WorkerConfig.from_env({})

        self.assertEqual(context.exception.env_name, "LUMA_FORGE_PROVISIONER_BEARER_TOKEN")
        self.assertEqual(context.exception.code, "configuration_error")
        self.assertEqual(context.exception.reason_code, "missing_required_value")
        self.assertNotIn(VALID_TOKEN, str(context.exception))

    def test_rejects_malformed_bearer_tokens_without_leaking_value(self):
        invalid_values = [
            "",
            "short-token",
            f"{VALID_TOKEN} with-space",
            f"{VALID_TOKEN}\n",
            f"{VALID_TOKEN}\x7f",
        ]

        for value in invalid_values:
            with self.subTest(value=repr(value)):
                with self.assertRaises(ConfigurationError) as context:
                    WorkerConfig.from_env(valid_env(LUMA_FORGE_PROVISIONER_BEARER_TOKEN=value))

                self.assertEqual(context.exception.env_name, "LUMA_FORGE_PROVISIONER_BEARER_TOKEN")
                self.assertEqual(context.exception.code, "configuration_error")
                if value:
                    self.assertNotIn(value, str(context.exception))

    def test_rejects_invalid_numeric_values(self):
        invalid_values = {
            "LUMA_FORGE_PROVISIONER_PORT": ["", "abc", "0", "65536"],
            "LUMA_FORGE_PROVISIONER_MAX_REQUEST_BYTES": ["", "abc", "0", str(MAX_REQUEST_BYTES_LIMIT + 1)],
            "LUMA_FORGE_PROVISIONER_GIT_TIMEOUT_SECONDS": ["", "abc", "0", "inf", str(MAX_TIMEOUT_SECONDS + 1)],
            "LUMA_FORGE_PROVISIONER_DEPENDENCY_TIMEOUT_SECONDS": [
                "",
                "abc",
                "0",
                "nan",
                str(MAX_TIMEOUT_SECONDS + 1),
            ],
            "LUMA_FORGE_PROVISIONER_DOWNLOAD_TIMEOUT_SECONDS": [
                "",
                "abc",
                "-1",
                "inf",
                str(MAX_TIMEOUT_SECONDS + 1),
            ],
        }

        for name, values in invalid_values.items():
            for value in values:
                with self.subTest(name=name, value=value):
                    with self.assertRaises(ConfigurationError) as context:
                        WorkerConfig.from_env(valid_env(**{name: value}))

                    self.assertEqual(context.exception.env_name, name)
                    self.assertEqual(context.exception.code, "configuration_error")

    def test_rejects_invalid_bind_host(self):
        for value in ["", "-bad-host", "bad_host", ".bad", "bad..host"]:
            with self.subTest(value=value):
                with self.assertRaises(ConfigurationError) as context:
                    WorkerConfig.from_env(valid_env(LUMA_FORGE_PROVISIONER_HOST=value))

                self.assertEqual(context.exception.env_name, "LUMA_FORGE_PROVISIONER_HOST")
                self.assertEqual(context.exception.code, "configuration_error")

    def test_rejects_invalid_workspace_mount_path(self):
        for value in ["", "workspace", "/workspace/../other", "/workspace/./other"]:
            with self.subTest(value=value):
                with self.assertRaises(ConfigurationError) as context:
                    WorkerConfig.from_env(valid_env(LUMA_FORGE_WORKSPACE_MOUNT_PATH=value))

                self.assertEqual(context.exception.env_name, "LUMA_FORGE_WORKSPACE_MOUNT_PATH")
                self.assertEqual(context.exception.code, "configuration_error")

    def test_configuration_error_payload_is_machine_readable_without_secret(self):
        error = ConfigurationError(
            "LUMA_FORGE_PROVISIONER_BEARER_TOKEN",
            "value_too_short",
            "value must be at least 32 characters",
        )

        payload = error.to_dict()

        self.assertEqual(payload["code"], "configuration_error")
        self.assertEqual(payload["env_name"], "LUMA_FORGE_PROVISIONER_BEARER_TOKEN")
        self.assertEqual(payload["reason_code"], "value_too_short")
        self.assertNotIn(VALID_TOKEN, json.dumps(payload))

    def test_main_prints_config_error_payload_and_exits_before_serving(self):
        error = ConfigurationError(
            "LUMA_FORGE_PROVISIONER_PORT",
            "invalid_integer",
            "value must be an integer",
        )
        stderr = StringIO()

        with patch("app.server.WorkerConfig.from_env", side_effect=error):
            with self.assertRaises(SystemExit) as context:
                with redirect_stderr(stderr):
                    main()

        self.assertEqual(context.exception.code, 78)
        payload = json.loads(stderr.getvalue())
        self.assertEqual(payload["code"], "configuration_error")
        self.assertEqual(payload["env_name"], "LUMA_FORGE_PROVISIONER_PORT")
        self.assertEqual(payload["reason_code"], "invalid_integer")

    def test_create_server_uses_validated_config(self):
        config = WorkerConfig.from_env(
            valid_env(
                LUMA_FORGE_PROVISIONER_HOST="127.0.0.1",
                LUMA_FORGE_PROVISIONER_PORT=str(_free_port()),
            )
        )
        server = create_server(config)
        try:
            self.assertEqual(server.server_address[0], config.host)
            self.assertEqual(server.server_address[1], config.port)
        finally:
            server.server_close()


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


if __name__ == "__main__":
    unittest.main()
