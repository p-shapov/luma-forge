import tempfile
import unittest
from pathlib import Path
from threading import Event

from app.errors import PreparationError
from app.schemas import parse_start_request
from helpers import start_payload, test_config
from runtime.manifest import runtime_paths
from runtime.materializer import RuntimeMaterializer


class MaterializerTests(unittest.TestCase):
    def test_rejects_mismatched_runtime_contract_metadata(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            config = test_config(workspace_mount_path=Path(directory), runtime_contract_id="other-runtime")

            with self.assertRaises(PreparationError):
                RuntimeMaterializer(config).materialize(
                    request.resolved_runtime_implementation,
                    runtime_paths(Path(directory)),
                    Event(),
                )

    def test_rejects_mismatched_provisioner_image_ref(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            config = test_config(
                workspace_mount_path=Path(directory),
                provisioner_image_ref="ghcr.io/luma-forge/provisioner-worker@sha256:9999999999999999999999999999999999999999999999999999999999999999",
            )

            with self.assertRaises(PreparationError):
                RuntimeMaterializer(config).materialize(
                    request.resolved_runtime_implementation,
                    runtime_paths(Path(directory)),
                    Event(),
                )

    def test_materializes_upstream_comfyui_custom_nodes_from_archive(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            paths = runtime_paths(Path(directory))

            RuntimeMaterializer(test_config(workspace_mount_path=Path(directory))).materialize(
                request.resolved_runtime_implementation,
                paths,
                Event(),
            )

            self.assertTrue((paths.comfyui_root / "custom_nodes" / "websocket_image_save.py").is_file())


if __name__ == "__main__":
    unittest.main()
