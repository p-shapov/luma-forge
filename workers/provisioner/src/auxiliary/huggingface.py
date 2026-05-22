from collections.abc import Callable
from dataclasses import dataclass
from multiprocessing import get_context
from pathlib import Path
from queue import Empty
from threading import Event
from time import monotonic

from auxiliary.cancellation import Cancelled
from app.errors import (
    AssetAuthRequiredError,
    AssetDownloadError,
    StepTimeoutError,
    WorkerError,
)
from app.schemas import HuggingFaceSource, ModelAsset

HubDownload = Callable[..., str]


@dataclass(frozen=True)
class PublicFileDownloader:
    hub_download: HubDownload | None = None

    def download(
        self,
        asset: ModelAsset,
        target: Path,
        *,
        cancel_event: Event | None = None,
        timeout_seconds: float | None = None,
    ) -> None:
        target.parent.mkdir(parents=True, exist_ok=True)
        source = asset.download_source
        try:
            cached_path = _download_with_isolated_process(
                source,
                target.parent,
                self.hub_download,
                timeout_seconds=timeout_seconds,
                cancel_event=cancel_event,
            )
            self._place_downloaded_file(Path(cached_path), target, cancel_event=cancel_event)
        except Cancelled:
            raise
        except WorkerError:
            raise
        except Exception as error:
            if _is_huggingface_auth_error(error):
                raise AssetAuthRequiredError("Hugging Face asset requires authentication.") from error
            raise AssetDownloadError("Hugging Face asset download failed.") from error

    def _place_downloaded_file(self, downloaded_path: Path, target: Path, *, cancel_event: Event | None) -> None:
        if downloaded_path.resolve(strict=False) == target.resolve(strict=False):
            return

        temporary = target.with_suffix(target.suffix + ".part")
        try:
            with downloaded_path.open("rb") as source, temporary.open("wb") as output:
                while True:
                    if cancel_event is not None and cancel_event.is_set():
                        raise Cancelled()
                    chunk = source.read(1024 * 1024)
                    if not chunk:
                        break
                    output.write(chunk)
            temporary.replace(target)
        finally:
            if temporary.exists():
                temporary.unlink()


def _load_hub_download() -> HubDownload:
    try:
        from huggingface_hub import hf_hub_download
    except ImportError as error:
        raise AssetDownloadError("Hugging Face Hub client is unavailable.") from error
    return hf_hub_download


def _download_with_isolated_process(
    source: HuggingFaceSource,
    local_dir: Path,
    hub_download: HubDownload | None,
    *,
    timeout_seconds: float | None,
    cancel_event: Event | None,
) -> str:
    if timeout_seconds is None:
        return _download_from_hub(source, local_dir, hub_download)

    context = get_context("spawn")
    result_queue = context.Queue()
    process = context.Process(
        target=_hub_download_process,
        args=(source, str(local_dir), hub_download, result_queue),
    )
    process.start()

    deadline = monotonic() + timeout_seconds
    while process.is_alive():
        if cancel_event is not None and cancel_event.is_set():
            _terminate_process(process)
            raise Cancelled()
        if monotonic() >= deadline:
            _terminate_process(process)
            raise StepTimeoutError("Hugging Face asset download timed out.")
        process.join(timeout=0.1)

    try:
        status, *payload = result_queue.get(timeout=1)
    except Empty as error:
        raise AssetDownloadError("Hugging Face asset download failed.") from error

    if status == "ok":
        return payload[0]

    error_class, status_code, message = payload
    if error_class == "GatedRepoError" or status_code in (401, 403):
        raise AssetAuthRequiredError("Hugging Face asset requires authentication.")
    raise AssetDownloadError("Hugging Face asset download failed.") from RuntimeError(message)


def _download_from_hub(source: HuggingFaceSource, local_dir: Path, hub_download: HubDownload | None) -> str:
    download = hub_download or _load_hub_download()
    return download(
        repo_id=source.repository_id,
        filename=source.file_path,
        revision=source.revision,
        repo_type="model",
        local_dir=str(local_dir),
        token=False,
    )


def _hub_download_process(
    source: HuggingFaceSource,
    local_dir: str,
    hub_download: HubDownload | None,
    result_queue,
) -> None:
    try:
        result_queue.put(("ok", _download_from_hub(source, Path(local_dir), hub_download)))
    except BaseException as error:
        response = getattr(error, "response", None)
        status_code = getattr(response, "status_code", None)
        result_queue.put(("error", error.__class__.__name__, status_code, str(error)))


def _terminate_process(process) -> None:
    process.terminate()
    process.join(timeout=5)
    if process.is_alive():
        process.kill()
        process.join(timeout=5)


def _is_huggingface_auth_error(error: Exception) -> bool:
    if error.__class__.__name__ == "GatedRepoError":
        return True
    response = getattr(error, "response", None)
    status_code = getattr(response, "status_code", None)
    return status_code in (401, 403)
