import tempfile
import time
import unittest
from io import BytesIO
from pathlib import Path

from helpers import start_payload
from auxiliary.huggingface import PublicFileDownloader
from app.errors import AssetAuthRequiredError, AssetDownloadError, StepTimeoutError
from app.schemas import parse_start_request


class FakeHubUrl:
    def __init__(self):
        self.calls = []

    def __call__(self, **kwargs):
        self.calls.append(kwargs)
        return (
            f"https://huggingface.test/{kwargs['repo_id']}/resolve/"
            f"{kwargs['revision']}/{kwargs['filename']}"
        )


class FakeUrlOpen:
    def __init__(self, content: bytes = b"model"):
        self.content = content
        self.requests = []

    def __call__(self, request):
        self.requests.append(request)
        return BytesIO(self.content)


class BrokenStream:
    def __init__(self):
        self.reads = 0

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        return False

    def read(self, size=-1):
        self.reads += 1
        if self.reads == 1:
            return b"partial"
        raise RuntimeError("stream failed")


class SlowUrlOpen:
    def __call__(self, request):
        time.sleep(10)
        return BytesIO(b"late")


class PublicFileDownloaderTests(unittest.TestCase):
    def test_downloads_asset_with_huggingface_hub_client(self):
        request = parse_start_request(start_payload())
        asset = request.workflow_preset.required_model_assets[0]
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "models/checkpoints/model.safetensors"
            hub_url = FakeHubUrl()
            open_url = FakeUrlOpen()

            PublicFileDownloader(hub_url, open_url).download(
                asset,
                target,
                timeout_seconds=None,
            )

            self.assertEqual(hub_url.calls[0]["repo_id"], "owner/model")
            self.assertEqual(hub_url.calls[0]["filename"], "model.safetensors")
            self.assertEqual(hub_url.calls[0]["revision"], "main")
            self.assertEqual(
                open_url.requests[0].full_url,
                "https://huggingface.test/owner/model/resolve/main/model.safetensors",
            )
            self.assertTrue(target.is_file())
            self.assertEqual(target.read_bytes(), b"model")

    def test_uses_configured_token_for_authenticated_asset(self):
        request = parse_start_request(start_payload())
        asset = request.workflow_preset.required_model_assets[0]
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "models/checkpoints/model.safetensors"
            open_url = FakeUrlOpen()

            PublicFileDownloader(FakeHubUrl(), open_url).download(
                asset,
                target,
                timeout_seconds=None,
                hugging_face_api_key="test-hugging-face-key",
            )

            self.assertEqual(
                open_url.requests[0].headers["Authorization"],
                "Bearer test-hugging-face-key",
            )
            self.assertTrue(target.is_file())

    def test_download_writes_only_requested_target_when_source_path_has_directory(self):
        payload = start_payload()
        payload["workflow_preset"]["required_model_assets"][0]["download_source"][
            "file_path"
        ] = "text_encoders/model.safetensors"
        payload["workflow_preset"]["required_model_assets"][0][
            "install_comfyui_relative_path"
        ] = "models/text_encoders/model.safetensors"
        asset = parse_start_request(payload).workflow_preset.required_model_assets[0]

        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "models/text_encoders/model.safetensors"
            duplicate = Path(directory) / "models/text_encoders/text_encoders/model.safetensors"

            PublicFileDownloader(FakeHubUrl(), FakeUrlOpen()).download(
                asset,
                target,
                timeout_seconds=None,
            )

            self.assertTrue(target.is_file())
            self.assertFalse(duplicate.exists())

    def test_removes_partial_file_when_download_fails(self):
        request = parse_start_request(start_payload())
        asset = request.workflow_preset.required_model_assets[0]

        def fail_download(request):
            return BrokenStream()

        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "models/checkpoints/model.safetensors"

            with self.assertRaises(AssetDownloadError):
                PublicFileDownloader(FakeHubUrl(), fail_download).download(
                    asset,
                    target,
                    timeout_seconds=None,
                )

            self.assertFalse(target.exists())
            self.assertFalse(target.with_suffix(target.suffix + ".part").exists())

    def test_maps_huggingface_auth_failure(self):
        def fail_auth(request):
            error = RuntimeError("forbidden")
            error.response = type("Response", (), {"status_code": 403})()
            raise error

        request = parse_start_request(start_payload())
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(AssetAuthRequiredError):
                PublicFileDownloader(FakeHubUrl(), fail_auth).download(
                    request.workflow_preset.required_model_assets[0],
                    Path(directory) / "model.safetensors",
                    timeout_seconds=None,
                )

    def test_maps_huggingface_download_failure(self):
        def fail_download(request):
            raise RuntimeError("missing")

        request = parse_start_request(start_payload())
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(AssetDownloadError):
                PublicFileDownloader(FakeHubUrl(), fail_download).download(
                    request.workflow_preset.required_model_assets[0],
                    Path(directory) / "model.safetensors",
                    timeout_seconds=None,
                )

    def test_download_timeout_terminates_hub_process(self):
        request = parse_start_request(start_payload())
        asset = request.workflow_preset.required_model_assets[0]
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "models/checkpoints/model.safetensors"

            with self.assertRaises(StepTimeoutError):
                PublicFileDownloader(FakeHubUrl(), SlowUrlOpen()).download(
                    asset,
                    target,
                    timeout_seconds=0.1,
            )

            time.sleep(0.3)
            self.assertFalse(target.exists())
            self.assertFalse(target.with_suffix(target.suffix + ".part").exists())


if __name__ == "__main__":
    unittest.main()
