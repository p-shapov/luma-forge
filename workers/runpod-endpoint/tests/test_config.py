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
