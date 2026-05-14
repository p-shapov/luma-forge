import unittest
from pathlib import Path

from runpod_endpoint_worker.config import EndpointConfig
from runpod_endpoint_worker.environment import safe_child_path, validate_prepared_environment
from runpod_endpoint_worker.errors import PreparedEnvironmentError, ValidationError
from helpers import WorkerFixture


class EnvironmentTests(unittest.TestCase):
    def test_validates_prepared_environment(self):
        with WorkerFixture() as fixture:
            validate_prepared_environment(fixture.config)

    def test_fails_when_comfyui_entrypoint_is_missing(self):
        with WorkerFixture() as fixture:
            (fixture.comfyui_root / "main.py").unlink()

            with self.assertRaises(PreparedEnvironmentError):
                validate_prepared_environment(fixture.config)

    def test_fails_when_workflow_is_missing(self):
        with WorkerFixture() as fixture:
            (fixture.comfyui_root / "workflows/t2i.json").unlink()

            with self.assertRaises(PreparedEnvironmentError):
                validate_prepared_environment(fixture.config)

    def test_fails_when_required_model_is_missing(self):
        with WorkerFixture() as fixture:
            (fixture.comfyui_root / "models/checkpoints/sd_xl_base_1.0.safetensors").unlink()

            with self.assertRaises(PreparedEnvironmentError):
                validate_prepared_environment(fixture.config)

    def test_safe_child_path_rejects_parent_traversal(self):
        with self.assertRaises(ValidationError):
            safe_child_path(Path("/workspace/ComfyUI"), Path("../bad"), "field")

    def test_validates_custom_node_path_when_configured(self):
        with WorkerFixture() as fixture:
            (fixture.comfyui_root / "custom_nodes/node").mkdir(parents=True)
            config = EndpointConfig(
                workspace_mount_path=fixture.workspace,
                required_custom_node_paths=(Path("custom_nodes/node"),),
            )

            validate_prepared_environment(config)


if __name__ == "__main__":
    unittest.main()
