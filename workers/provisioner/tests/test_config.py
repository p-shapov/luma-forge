from contextlib import redirect_stderr
from dataclasses import replace
from io import StringIO
import json
import unittest
from unittest.mock import patch
from pathlib import Path

from app.config import (
    ConfigurationError,
    DOWNLOAD_INACTIVITY_TIMEOUT_SECONDS,
    HOST,
    PORT,
    REQUIRED_MODEL_ASSETS_ENV,
    WORKSPACE_MOUNT_PATH,
    WorkerConfig,
)
from app.server import create_server, main


VALID_TOKEN = "config-token-0123456789abcdef0123"


def valid_env(**overrides):
    env = {
        "LUMA_FORGE_PROVISIONER_BEARER_TOKEN": VALID_TOKEN,
        REQUIRED_MODEL_ASSETS_ENV: r'[{"id":"model","name":"Model","download_source":{"source_type":"huggingface","repository_id":"owner/model","file_path":"model.safetensors","revision":"main"},"install_comfyui_relative_path":"models/checkpoints/model.safetensors"}]',
    }
    env.update(overrides)
    return env


class ConfigTests(unittest.TestCase):
    def test_valid_config_uses_defaults_for_optional_values(self):
        config = WorkerConfig.from_env(valid_env())

        self.assertEqual(config.host, HOST)
        self.assertEqual(config.port, PORT)
        self.assertEqual(config.bearer_token, VALID_TOKEN)
        self.assertEqual(
            config.download_inactivity_timeout_seconds,
            DOWNLOAD_INACTIVITY_TIMEOUT_SECONDS,
        )
        self.assertEqual(config.workspace_mount_path, Path(WORKSPACE_MOUNT_PATH).resolve(strict=False))

    def test_valid_config_uses_baked_values_and_hugging_face_env(self):
        config = WorkerConfig.from_env(
            valid_env(
                LUMA_FORGE_HUGGING_FACE_API_KEY="test-hugging-face-key",
            )
        )

        self.assertEqual(config.host, HOST)
        self.assertEqual(config.port, PORT)
        self.assertEqual(
            config.download_inactivity_timeout_seconds,
            DOWNLOAD_INACTIVITY_TIMEOUT_SECONDS,
        )
        self.assertEqual(config.workspace_mount_path, Path(WORKSPACE_MOUNT_PATH).resolve(strict=False))
        self.assertEqual(config.hugging_face_api_key, "test-hugging-face-key")

    def test_optional_hugging_face_api_key_absent_or_blank(self):
        self.assertIsNone(WorkerConfig.from_env(valid_env()).hugging_face_api_key)
        self.assertIsNone(
            WorkerConfig.from_env(
                valid_env(LUMA_FORGE_HUGGING_FACE_API_KEY="  ")
            ).hugging_face_api_key
        )

    def test_rejects_missing_bearer_token_without_leaking_value(self):
        with self.assertRaises(ConfigurationError) as context:
            WorkerConfig.from_env({})

        self.assertEqual(context.exception.env_name, "LUMA_FORGE_PROVISIONER_BEARER_TOKEN")
        self.assertEqual(context.exception.code, "missing_required_value")
        self.assertNotIn(VALID_TOKEN, str(context.exception))

    def test_rejects_missing_start_request_without_leaking_value(self):
        with self.assertRaises(ConfigurationError) as context:
            WorkerConfig.from_env(
                {
                    "LUMA_FORGE_PROVISIONER_BEARER_TOKEN": VALID_TOKEN,
                }
            )

        self.assertEqual(context.exception.env_name, REQUIRED_MODEL_ASSETS_ENV)
        self.assertEqual(context.exception.code, "missing_required_value")

    def test_rejects_malformed_bearer_tokens_without_leaking_value(self):
        invalid_values = [
            "",
            "short-token",
            f"{VALID_TOKEN} with-space",
            f"{VALID_TOKEN}\n",
            f"{VALID_TOKEN}\x7f",
            f"{VALID_TOKEN}é",
        ]

        for value in invalid_values:
            with self.subTest(value=repr(value)):
                with self.assertRaises(ConfigurationError) as context:
                    WorkerConfig.from_env(valid_env(LUMA_FORGE_PROVISIONER_BEARER_TOKEN=value))

                self.assertEqual(context.exception.env_name, "LUMA_FORGE_PROVISIONER_BEARER_TOKEN")
                if value:
                    self.assertNotIn(value, str(context.exception))

    def test_configuration_error_payload_is_machine_readable_without_secret(self):
        error = ConfigurationError(
            "LUMA_FORGE_PROVISIONER_BEARER_TOKEN",
            "value_too_short",
            "value must be at least 32 characters",
        )

        payload = error.to_dict()

        self.assertEqual(payload["code"], "value_too_short")
        self.assertEqual(payload["env_name"], "LUMA_FORGE_PROVISIONER_BEARER_TOKEN")
        self.assertNotIn(VALID_TOKEN, json.dumps(payload))

    def test_main_prints_config_error_payload_and_exits_before_serving(self):
        error = ConfigurationError(
            REQUIRED_MODEL_ASSETS_ENV,
            "invalid_json",
            "value must be valid JSON",
        )
        stderr = StringIO()

        with patch("app.server.WorkerConfig.from_env", side_effect=error):
            with self.assertRaises(SystemExit) as context:
                with redirect_stderr(stderr):
                    main()

        self.assertEqual(context.exception.code, 78)
        payload = json.loads(stderr.getvalue())
        self.assertEqual(payload["code"], "invalid_json")
        self.assertEqual(payload["env_name"], REQUIRED_MODEL_ASSETS_ENV)

    def test_create_server_uses_validated_config(self):
        config = replace(WorkerConfig.from_env(valid_env()), port=0)
        server = create_server(config)
        try:
            self.assertEqual(server.server_address[0], config.host)
            self.assertGreater(server.server_address[1], 0)
        finally:
            server.server_close()


if __name__ == "__main__":
    unittest.main()
