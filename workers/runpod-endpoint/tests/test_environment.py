import unittest
from pathlib import Path

from runpod_endpoint_worker.environment import safe_child_path, validate_prepared_environment
from runpod_endpoint_worker.errors import PreparedEnvironmentError, PreparedRuntimeError, ValidationError
from helpers import WorkerFixture


class EnvironmentTests(unittest.TestCase):
    def test_validates_prepared_environment(self):
        with WorkerFixture() as fixture:
            runtime = validate_prepared_environment(fixture.config)

            self.assertEqual(runtime.workspace_root, fixture.workspace)

    def test_fails_when_runtime_manifest_is_missing(self):
        with WorkerFixture() as fixture:
            fixture.config.runtime_manifest_path.unlink()

            with self.assertRaises(PreparedRuntimeError):
                validate_prepared_environment(fixture.config)

    def test_fails_when_manifest_kind_is_invalid(self):
        with WorkerFixture() as fixture:
            payload = fixture.config.runtime_manifest_path.read_text(encoding="utf-8")
            fixture.config.runtime_manifest_path.write_text(
                payload.replace('"luma_forge_prepared_workspace"', '"container_python"'),
                encoding="utf-8",
            )

            with self.assertRaises(PreparedRuntimeError):
                validate_prepared_environment(fixture.config)

    def test_fails_when_runtime_manifest_is_not_object(self):
        with WorkerFixture() as fixture:
            fixture.config.runtime_manifest_path.write_text("[]", encoding="utf-8")

            with self.assertRaises(PreparedRuntimeError):
                validate_prepared_environment(fixture.config)

    def test_fails_when_venv_interpreter_is_missing(self):
        with WorkerFixture() as fixture:
            fixture.venv_python.unlink()

            with self.assertRaises(PreparedEnvironmentError):
                validate_prepared_environment(fixture.config)

    def test_fails_when_comfyui_entrypoint_is_missing(self):
        with WorkerFixture() as fixture:
            (fixture.comfyui_root / "main.py").unlink()

            with self.assertRaises(PreparedEnvironmentError):
                validate_prepared_environment(fixture.config)

    def test_fails_when_workflow_is_missing(self):
        with WorkerFixture() as fixture:
            (fixture.workspace / "workflows/t2i.json").unlink()

            with self.assertRaises(PreparedEnvironmentError):
                validate_prepared_environment(fixture.config)

    def test_fails_when_required_model_is_missing(self):
        with WorkerFixture() as fixture:
            (fixture.workspace / "models/checkpoints/sd_xl_base_1.0.safetensors").unlink()

            with self.assertRaises(PreparedEnvironmentError):
                validate_prepared_environment(fixture.config)

    def test_runtime_manifest_does_not_require_dependency_overlay_records(self):
        with WorkerFixture() as fixture:
            runtime = validate_prepared_environment(fixture.config)

            self.assertEqual(runtime.model_asset_paths[0], fixture.workspace / "models/checkpoints/sd_xl_base_1.0.safetensors")

    def test_safe_child_path_rejects_parent_traversal(self):
        with self.assertRaises(ValidationError):
            safe_child_path(Path("/workspace/ComfyUI"), Path("../bad"), "field")


if __name__ == "__main__":
    unittest.main()
