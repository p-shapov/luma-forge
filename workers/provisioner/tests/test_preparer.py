import json
import tempfile
import unittest
from pathlib import Path
from threading import Event

from auxiliary.cancellation import Cancelled
from app.errors import PreparationError, ValidationError
from app.schemas import parse_start_request
from helpers import start_payload, test_config
from orchestration.preparer import Provisioner


class FakeDownloader:
    def __init__(self):
        self.calls = []

    def download(self, asset, target, *, cancel_event=None, timeout_seconds=None):
        self.calls.append((asset, target, timeout_seconds))
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(b"model")


class CancelAwareFakeDownloader(FakeDownloader):
    def download(self, asset, target, *, cancel_event=None, timeout_seconds=None):
        if cancel_event is not None and cancel_event.is_set():
            raise Cancelled()
        super().download(asset, target, cancel_event=cancel_event, timeout_seconds=timeout_seconds)


class MissingFileDownloader:
    def download(self, asset, target, *, cancel_event=None, timeout_seconds=None):
        target.parent.mkdir(parents=True, exist_ok=True)


class PreparerTests(unittest.TestCase):
    def test_prepares_workspace_and_downloads_models_without_endpoint_runtime(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            downloader = FakeDownloader()
            phases = []

            Provisioner(
                downloader=downloader,
                config=test_config(workspace_mount_path=Path(directory)),
            ).prepare(
                request,
                lambda phase, progress, message: phases.append(phase),
                Event(),
            )

            workspace = Path(directory)
            manifest_path = workspace / ".luma-forge/runtime-manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))

            self.assertIn("preparing_workspace", phases)
            self.assertIn("downloading_assets", phases)
            self.assertIn("validating_environment", phases)
            self.assertTrue((workspace / "models/checkpoints/model.safetensors").is_file())
            self.assertTrue((workspace / "output").is_dir())
            self.assertTrue((workspace / "workflows").is_dir())
            self.assertTrue(manifest_path.is_file())
            self.assertEqual(manifest["manifest_kind"], "luma_forge_prepared_workspace")
            self.assertIn(
                str((workspace / "models/checkpoints/model.safetensors").resolve(strict=False)),
                manifest["model_asset_paths"],
            )
            self.assertNotIn("python_overlay_path", manifest)
            self.assertNotIn("custom_node_revisions", manifest)
            self.assertNotIn("overlay_dependency_record_paths", manifest)
            self.assertNotIn("python_path", manifest)
            self.assertNotIn("comfyui_root", manifest)
            self.assertNotIn("image_runtime_root", manifest)
            self.assertFalse((workspace / ".venv/bin/python").exists())
            self.assertFalse((workspace / "ComfyUI").exists())
            self.assertFalse((workspace / "custom_nodes").exists())
            self.assertFalse((workspace / ".luma-forge/python-overlay").exists())

    def test_rejects_unsafe_preset_identifiers_without_echoing_values(self):
        unsafe_values = [
            "../unsafe-token",
            " unsafe-token",
            "bad id",
            "bad/id",
            "bad<id>",
            "badé",
            "a" * 129,
        ]

        for unsafe_value in unsafe_values:
            cases = [
                ("workflow_preset.id", lambda payload: payload["workflow_preset"].update({"id": unsafe_value})),
                (
                    "model_asset.id",
                    lambda payload: payload["workflow_preset"].update(
                        {"required_model_assets": [model_asset(id=unsafe_value)]}
                    ),
                ),
            ]
            for field, mutate in cases:
                with self.subTest(field=field, unsafe_value=unsafe_value):
                    payload = start_payload()
                    mutate(payload)

                    with self.assertRaises(ValidationError) as context:
                        parse_start_request(payload)

                    self.assertNotIn(unsafe_value, str(context.exception))

    def test_rejects_unsafe_huggingface_file_path(self):
        for file_path in ["/tmp/model.safetensors", "../model.safetensors", "models/../model.safetensors"]:
            with self.subTest(file_path=file_path):
                payload = start_payload()
                payload["workflow_preset"]["required_model_assets"][0]["download_source"]["file_path"] = file_path

                with self.assertRaises(ValidationError):
                    parse_start_request(payload)

    def test_rejects_missing_downloaded_model_file(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())

            with self.assertRaises(PreparationError):
                Provisioner(
                    downloader=MissingFileDownloader(),
                    config=test_config(workspace_mount_path=Path(directory)),
                ).prepare(request, lambda phase, progress, message: None, Event())

    def test_cancel_before_prepare_stops_without_creating_workspace(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            cancel = Event()
            cancel.set()

            with self.assertRaises(Cancelled):
                Provisioner(
                    config=test_config(workspace_mount_path=Path(directory)),
                ).prepare(request, lambda phase, progress, message: None, cancel)

            self.assertEqual(list(Path(directory).iterdir()), [])

    def test_cancel_during_download_stops_before_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            cancel = Event()
            phases = []

            class CancellingDownloader(CancelAwareFakeDownloader):
                def download(self, asset, target, *, cancel_event=None, timeout_seconds=None):
                    cancel.set()
                    super().download(asset, target, cancel_event=cancel_event, timeout_seconds=timeout_seconds)

            with self.assertRaises(Cancelled):
                Provisioner(
                    downloader=CancellingDownloader(),
                    config=test_config(workspace_mount_path=Path(directory)),
                ).prepare(request, lambda phase, progress, message: phases.append(phase), cancel)

            self.assertIn("downloading_assets", phases)
            self.assertFalse((Path(directory) / ".luma-forge/runtime-manifest.json").exists())


def model_asset(*, id: str = "model") -> dict:
    asset = start_payload()["workflow_preset"]["required_model_assets"][0].copy()
    asset["id"] = id
    return asset


if __name__ == "__main__":
    unittest.main()
