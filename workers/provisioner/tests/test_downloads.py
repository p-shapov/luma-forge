import tempfile
import time
import unittest
from pathlib import Path
from threading import Event

from helpers import start_payload
from auxiliary.huggingface import PublicFileDownloader
from app.errors import AssetAuthRequiredError, AssetDownloadError, StepTimeoutError
from app.schemas import parse_start_request


class FakeHubDownload:
    def __init__(self):
        self.calls = []

    def __call__(self, **kwargs):
        self.calls.append(kwargs)
        local_path = Path(kwargs["local_dir"]) / kwargs["filename"]
        local_path.parent.mkdir(parents=True, exist_ok=True)
        local_path.write_bytes(b"model")
        return str(local_path)


class SlowHubDownload:
    def __call__(self, **kwargs):
        time.sleep(10)
        local_path = Path(kwargs["local_dir"]) / kwargs["filename"]
        local_path.parent.mkdir(parents=True, exist_ok=True)
        local_path.write_bytes(b"late")
        return str(local_path)


class PublicFileDownloaderTests(unittest.TestCase):
    def test_downloads_asset_with_huggingface_hub_client(self):
        request = parse_start_request(start_payload())
        asset = request.workflow_preset.required_model_assets[0]
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "models/checkpoints/model.safetensors"
            hub_download = FakeHubDownload()

            PublicFileDownloader(hub_download).download(asset, target, cancel_event=Event(), timeout_seconds=None)

            self.assertEqual(hub_download.calls[0]["repo_id"], "owner/model")
            self.assertEqual(hub_download.calls[0]["filename"], "model.safetensors")
            self.assertEqual(hub_download.calls[0]["revision"], "main")
            self.assertFalse(hub_download.calls[0]["token"])
            self.assertTrue(target.is_file())

    def test_uses_hub_returned_target_for_cache_reuse(self):
        request = parse_start_request(start_payload())
        asset = request.workflow_preset.required_model_assets[0]
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "models/checkpoints/model.safetensors"
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(b"cached")
            calls = []

            def cached_download(**kwargs):
                calls.append(kwargs)
                return str(target)

            PublicFileDownloader(cached_download).download(asset, target, cancel_event=Event(), timeout_seconds=None)

            self.assertEqual(len(calls), 1)
            self.assertEqual(target.read_bytes(), b"cached")

    def test_maps_huggingface_auth_failure(self):
        def fail_auth(**kwargs):
            error = RuntimeError("forbidden")
            error.response = type("Response", (), {"status_code": 403})()
            raise error

        request = parse_start_request(start_payload())
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(AssetAuthRequiredError):
                PublicFileDownloader(fail_auth).download(
                    request.workflow_preset.required_model_assets[0],
                    Path(directory) / "model.safetensors",
                    cancel_event=Event(),
                    timeout_seconds=None,
                )

    def test_maps_huggingface_download_failure(self):
        def fail_download(**kwargs):
            raise RuntimeError("missing")

        request = parse_start_request(start_payload())
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(AssetDownloadError):
                PublicFileDownloader(fail_download).download(
                    request.workflow_preset.required_model_assets[0],
                    Path(directory) / "model.safetensors",
                    cancel_event=Event(),
                    timeout_seconds=None,
                )

    def test_download_timeout_terminates_hub_process(self):
        request = parse_start_request(start_payload())
        asset = request.workflow_preset.required_model_assets[0]
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "models/checkpoints/model.safetensors"

            with self.assertRaises(StepTimeoutError):
                PublicFileDownloader(SlowHubDownload()).download(
                    asset,
                    target,
                    cancel_event=Event(),
                    timeout_seconds=0.1,
                )

            time.sleep(0.3)
            self.assertFalse(target.exists())


if __name__ == "__main__":
    unittest.main()
