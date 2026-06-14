from unittest import TestCase

from runpod_endpoint_worker.config import (
    COMFY_CLI_PATH,
    COMFY_UI_READY_POLL_SECONDS,
    COMFYUI_HOST,
    COMFYUI_PATH,
    COMFYUI_PORT,
    COMFYUI_STARTUP_TIMEOUT_SECONDS,
    EXECUTION_CONTRACT_PATH,
    EXECUTION_TIMEOUT_SECONDS,
    EndpointConfig,
    MAX_ARTIFACT_BYTES,
    MAX_RESPONSE_BYTES,
    WORKFLOW_PATH,
    WORKSPACE_MOUNT_PATH,
)


class EndpointConfigTests(TestCase):
    def test_uses_baked_runtime_configuration(self):
        config = EndpointConfig.from_env()

        self.assertEqual(config.workspace_mount_path, WORKSPACE_MOUNT_PATH)
        self.assertEqual(config.comfy_cli_path, COMFY_CLI_PATH)
        self.assertEqual(config.comfyui_path, COMFYUI_PATH)
        self.assertEqual(config.workflow_path, WORKFLOW_PATH)
        self.assertEqual(config.execution_contract_path, EXECUTION_CONTRACT_PATH)
        self.assertEqual(config.comfyui_host, COMFYUI_HOST)
        self.assertEqual(config.comfyui_port, COMFYUI_PORT)
        self.assertEqual(config.comfyui_startup_timeout_seconds, COMFYUI_STARTUP_TIMEOUT_SECONDS)
        self.assertEqual(config.comfy_ui_ready_poll_seconds, COMFY_UI_READY_POLL_SECONDS)
        self.assertEqual(config.execution_timeout_seconds, EXECUTION_TIMEOUT_SECONDS)
        self.assertEqual(config.max_response_bytes, MAX_RESPONSE_BYTES)
        self.assertEqual(config.max_artifact_bytes, MAX_ARTIFACT_BYTES)
