import tempfile
import unittest
from pathlib import Path

from app.errors import AssetAuthRequiredError, PreparationError, ValidationError
from app.schemas import parse_start_request
from helpers import start_payload, test_config
from orchestration.preparer import Provisioner


class FakeDownloader:
    def __init__(self):
        self.calls = []

    def download(self, asset, target, *, timeout_seconds=None, hugging_face_api_key=None):
        self.calls.append((asset, target, timeout_seconds, hugging_face_api_key))
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(b"model")


class MissingFileDownloader:
    def download(self, asset, target, *, timeout_seconds=None, hugging_face_api_key=None):
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
            )

            workspace = Path(directory)

            self.assertIn("preparing_workspace", phases)
            self.assertIn("downloading_assets", phases)
            self.assertIn("validating_environment", phases)
            self.assertTrue((workspace / "models/checkpoints/model.safetensors").is_file())
            self.assertTrue((workspace / "output").is_dir())
            self.assertTrue((workspace / "workflows").is_dir())
            self.assertFalse((workspace / ".luma-forge").exists())
            self.assertFalse((workspace / ".luma-forge/runtime-manifest.json").exists())
            self.assertFalse((workspace / ".venv/bin/python").exists())
            self.assertFalse((workspace / "ComfyUI").exists())
            self.assertFalse((workspace / "custom_nodes").exists())

    def test_download_progress_advances_after_each_asset_completes(self):
        with tempfile.TemporaryDirectory() as directory:
            preset = {
                "requires_hugging_face_api_key": False,
                "required_model_assets": [
                    model_asset(id="model-a", install_path="models/checkpoints/model-a.safetensors"),
                    model_asset(id="model-b", install_path="models/checkpoints/model-b.safetensors"),
                ]
            }
            request = parse_start_request(start_payload(preset=preset))
            events = []

            Provisioner(
                downloader=FakeDownloader(),
                config=test_config(workspace_mount_path=Path(directory)),
            ).prepare(
                request,
                lambda phase, progress, message: events.append((phase, progress, message)),
            )

            download_events = [
                (progress, message)
                for phase, progress, message in events
                if phase == "downloading_assets"
            ]
            self.assertEqual(
                download_events,
                [
                    (55, "Downloading model assets"),
                    (72, "Downloaded model asset Model"),
                    (90, "Downloaded model asset Model"),
                ],
            )

    def test_passes_hugging_face_key_to_downloads_when_workflow_requires_it(self):
        with tempfile.TemporaryDirectory() as directory:
            preset = {
                "requires_hugging_face_api_key": True,
                "required_model_assets": [
                    model_asset(id="model-a"),
                    model_asset(id="model-b"),
                ]
            }
            request = parse_start_request(start_payload(preset=preset))
            downloader = FakeDownloader()

            Provisioner(
                downloader=downloader,
                config=test_config(
                    workspace_mount_path=Path(directory),
                    hugging_face_api_key="test-hugging-face-key",
                ),
            ).prepare(request, lambda phase, progress, message: None)

            self.assertEqual(downloader.calls[0][3], "test-hugging-face-key")
            self.assertEqual(downloader.calls[1][3], "test-hugging-face-key")

    def test_rejects_unsafe_model_asset_identifiers_without_echoing_values(self):
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
                (
                    "model_asset.id",
                    lambda payload: payload["required_model_assets"].__setitem__(
                        0,
                        {
                            **payload["required_model_assets"][0],
                            "id": unsafe_value,
                        },
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
                payload["required_model_assets"][0]["download_source"]["file_path"] = file_path

                with self.assertRaises(ValidationError):
                    parse_start_request(payload)

    def test_rejects_missing_or_invalid_workflow_auth_requirement_flag(self):
        cases = [
            lambda payload: payload.pop("requires_hugging_face_api_key"),
            lambda payload: payload.update({"requires_hugging_face_api_key": "true"}),
        ]

        for mutate in cases:
            payload = start_payload()
            mutate(payload)

            with self.assertRaises(ValidationError):
                parse_start_request(payload)

    def test_missing_required_hugging_face_key_fails_with_asset_auth_required(self):
        with tempfile.TemporaryDirectory() as directory:
            preset = {
                "requires_hugging_face_api_key": True,
                "required_model_assets": [
                    model_asset(),
                ]
            }
            request = parse_start_request(start_payload(preset=preset))
            downloader = FakeDownloader()

            with self.assertRaises(AssetAuthRequiredError):
                Provisioner(
                    downloader=downloader,
                    config=test_config(workspace_mount_path=Path(directory)),
                ).prepare(request, lambda phase, progress, message: None)
            self.assertEqual(downloader.calls, [])

    def test_rejects_missing_downloaded_model_file(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())

            with self.assertRaises(PreparationError):
                Provisioner(
                    downloader=MissingFileDownloader(),
                    config=test_config(workspace_mount_path=Path(directory)),
                ).prepare(request, lambda phase, progress, message: None)


def model_asset(
    *,
    id: str = "model",
    install_path: str = "models/checkpoints/model.safetensors",
) -> dict:
    asset = start_payload()["required_model_assets"][0].copy()
    asset["download_source"] = asset["download_source"].copy()
    asset["id"] = id
    asset["install_comfyui_relative_path"] = install_path
    return asset


if __name__ == "__main__":
    unittest.main()
