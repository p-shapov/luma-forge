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

    def test_reads_image_runtime_identity(self):
        endpoint_ref = (
            "ghcr.io/luma-forge/runpod-endpoint-worker@sha256:"
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        )
        with patch.dict(
            os.environ,
            {
                "LUMA_FORGE_IMAGE_RUNTIME_ROOT": "/image-runtime",
                "LUMA_FORGE_RUNTIME_CONTRACT_ID": "contract",
                "LUMA_FORGE_RUNTIME_CONTRACT_VERSION": "2.0.0",
                "LUMA_FORGE_RUNTIME_IMPLEMENTATION_REVISION": "2026.05.17-001",
                "LUMA_FORGE_ENDPOINT_IMAGE_REF": endpoint_ref,
            },
            clear=True,
        ):
            config = EndpointConfig.from_env()

        self.assertEqual(config.image_runtime_root_path, Path("/image-runtime"))
        self.assertEqual(config.runtime_contract_id, "contract")
        self.assertEqual(config.runtime_contract_version, "2.0.0")
        self.assertEqual(config.runtime_implementation_revision, "2026.05.17-001")
        self.assertEqual(config.endpoint_image_ref, endpoint_ref)
