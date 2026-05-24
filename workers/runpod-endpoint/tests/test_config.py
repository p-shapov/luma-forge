import os
from pathlib import Path
from unittest import TestCase
from unittest.mock import patch

from runpod_endpoint_worker.config import EndpointConfig


class EndpointConfigTests(TestCase):
    def test_reads_shared_workspace_mount_path(self):
        with patch.dict(os.environ, {"LUMA_FORGE_WORKSPACE_MOUNT_PATH": "/shared-workspace"}, clear=True):
            config = EndpointConfig.from_env()

        self.assertEqual(config.workspace_mount_path, Path("/shared-workspace"))

    def test_endpoint_workspace_mount_path_overrides_shared_path(self):
        with patch.dict(
            os.environ,
            {
                "LUMA_FORGE_WORKSPACE_MOUNT_PATH": "/shared-workspace",
                "LUMA_FORGE_RUNPOD_ENDPOINT_WORKSPACE_MOUNT_PATH": "/endpoint-workspace",
            },
            clear=True,
        ):
            config = EndpointConfig.from_env()

        self.assertEqual(config.workspace_mount_path, Path("/endpoint-workspace"))

    def test_reads_comfy_runtime_configuration(self):
        with patch.dict(
            os.environ,
            {
                "LUMA_FORGE_RUNPOD_ENDPOINT_COMFY_CLI_PATH": "/runtime/bin/comfy",
                "LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_PATH": "/runtime/ComfyUI",
                "LUMA_FORGE_RUNPOD_ENDPOINT_WORKFLOW_PATH": "/runtime/workflows/workflow.json",
                "LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_HOST": "0.0.0.0",
                "LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_PORT": "8199",
                "LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_STARTUP_TIMEOUT_SECONDS": "12",
                "LUMA_FORGE_RUNPOD_ENDPOINT_EXECUTION_TIMEOUT_SECONDS": "34",
                "LUMA_FORGE_RUNPOD_ENDPOINT_MAX_RESPONSE_BYTES": "56",
                "LUMA_FORGE_RUNPOD_ENDPOINT_MAX_ARTIFACT_BYTES": "78",
            },
            clear=True,
        ):
            config = EndpointConfig.from_env()

        self.assertEqual(config.comfy_cli_path, Path("/runtime/bin/comfy"))
        self.assertEqual(config.comfyui_path, Path("/runtime/ComfyUI"))
        self.assertEqual(config.workflow_path, Path("/runtime/workflows/workflow.json"))
        self.assertEqual(config.comfyui_host, "0.0.0.0")
        self.assertEqual(config.comfyui_port, 8199)
        self.assertEqual(config.comfyui_startup_timeout_seconds, 12)
        self.assertEqual(config.execution_timeout_seconds, 34)
        self.assertEqual(config.max_response_bytes, 56)
        self.assertEqual(config.max_artifact_bytes, 78)
