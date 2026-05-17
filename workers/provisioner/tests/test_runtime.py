import json
from pathlib import Path
import tempfile
import unittest

from helpers import start_payload
from app.errors import PreparationError
from runtime.manifest import (
    ENVIRONMENT_KIND,
    build_manifest,
    load_manifest,
    runtime_paths,
    validate_manifest,
    write_manifest,
)
from app.schemas import parse_start_request


class RuntimeTests(unittest.TestCase):
    def test_runtime_paths_resolve_under_workspace(self):
        with tempfile.TemporaryDirectory() as directory:
            paths = runtime_paths(Path(directory))

            self.assertEqual(paths.python_overlay_path, Path(directory).resolve() / ".luma-forge/python-overlay")
            self.assertEqual(paths.comfyui_root, Path(directory).resolve())
            self.assertEqual(paths.runtime_manifest_path, Path(directory).resolve() / ".luma-forge/runtime-manifest.json")

    def test_runtime_paths_use_catalog_overlay_path(self):
        with tempfile.TemporaryDirectory() as directory:
            paths = runtime_paths(Path(directory), python_overlay_path=Path(".luma-forge/custom-overlay"))

            self.assertEqual(paths.python_overlay_path, Path(directory).resolve() / ".luma-forge/custom-overlay")

    def test_runtime_paths_use_catalog_image_runtime_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            image_root = Path(directory) / "runtime"
            paths = runtime_paths(
                Path(directory) / "workspace",
                image_runtime_root=image_root,
                image_python_interpreter_path=Path("/catalog/runtime/python/bin/python"),
                image_comfyui_root_path=Path("/catalog/runtime/app/ComfyUI"),
                declared_image_runtime_root_path=Path("/catalog/runtime"),
            )

            self.assertEqual(paths.image_python_path, image_root.resolve() / "python/bin/python")
            self.assertEqual(paths.python_path, image_root.resolve() / "python/bin/python")
            self.assertEqual(paths.image_comfyui_root, image_root.resolve() / "app/ComfyUI")

    def test_manifest_round_trips_without_secret_fields(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            paths = runtime_paths(Path(directory))
            manifest = build_manifest(
                request=request,
                paths=paths,
                python_version="Python 3.12.0",
            )

            write_manifest(manifest, paths.runtime_manifest_path)
            loaded = load_manifest(paths.runtime_manifest_path)

            self.assertEqual(loaded.environment_kind, ENVIRONMENT_KIND)
            self.assertEqual(loaded.python_path, str(paths.image_python_path))
            self.assertEqual(
                loaded.image_base_dependency_record_paths,
                [str((paths.image_runtime_root / "base-runtime/pip-freeze.txt").resolve(strict=False))],
            )
            payload = json.loads(paths.runtime_manifest_path.read_text(encoding="utf-8"))
            self.assertNotIn("token", payload)
            self.assertNotIn("secret", payload)
            self.assertNotIn("api_key", payload)

    def test_manifest_requires_environment_kind(self):
        with tempfile.TemporaryDirectory() as directory:
            paths = runtime_paths(Path(directory))
            paths.runtime_manifest_path.parent.mkdir(parents=True)
            paths.runtime_manifest_path.write_text("{}", encoding="utf-8")

            with self.assertRaises(PreparationError):
                load_manifest(paths.runtime_manifest_path)

    def test_validate_manifest_rejects_wrong_environment_kind(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            paths = runtime_paths(Path(directory))
            manifest = build_manifest(
                request=request,
                paths=paths,
                python_version="Python 3.12.0",
            )
            invalid = type(manifest)(
                environment_kind="container_python",
                python_path=manifest.python_path,
                comfyui_root=manifest.comfyui_root,
                image_runtime_root=manifest.image_runtime_root,
                workspace_root=manifest.workspace_root,
                python_overlay_path=manifest.python_overlay_path,
                python_version=manifest.python_version,
                platform=manifest.platform,
                comfyui_revision=manifest.comfyui_revision,
                runtime_contract_id=manifest.runtime_contract_id,
                runtime_contract_version=manifest.runtime_contract_version,
                implementation_revision=manifest.implementation_revision,
                provisioner_image_ref=manifest.provisioner_image_ref,
                endpoint_image_ref=manifest.endpoint_image_ref,
                custom_node_revisions=manifest.custom_node_revisions,
                image_base_dependency_record_paths=manifest.image_base_dependency_record_paths,
                overlay_dependency_record_paths=manifest.overlay_dependency_record_paths,
                model_asset_paths=manifest.model_asset_paths,
                protected_dependency_policy_version=manifest.protected_dependency_policy_version,
                prepared_at=manifest.prepared_at,
            )

            with self.assertRaises(PreparationError):
                validate_manifest(invalid, paths=paths)


if __name__ == "__main__":
    unittest.main()
