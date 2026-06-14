from collections.abc import Callable
from dataclasses import dataclass
from multiprocessing import get_context
from pathlib import Path
from queue import Empty
from time import monotonic
from urllib.error import HTTPError
from urllib.parse import urlparse
from urllib.request import HTTPRedirectHandler, Request, build_opener

from app.errors import (
    AssetAuthRequiredError,
    AssetDownloadError,
    StepTimeoutError,
    WorkerError,
)
from app.schemas import HuggingFaceSource, ModelAsset

CHUNK_SIZE = 1024 * 1024

HubUrl = Callable[..., str]
UrlOpen = Callable[..., object]


@dataclass(frozen=True)
class PublicFileDownloader:
    hub_url: HubUrl | None = None
    open_url: UrlOpen | None = None

    def download(
        self,
        asset: ModelAsset,
        target: Path,
        *,
        download_inactivity_timeout_seconds: float | None = None,
        hugging_face_api_key: str | None = None,
    ) -> None:
        target.parent.mkdir(parents=True, exist_ok=True)
        source = asset.download_source
        try:
            _download_with_isolated_process(
                source,
                target,
                self.hub_url,
                self.open_url,
                download_inactivity_timeout_seconds=download_inactivity_timeout_seconds,
                hugging_face_api_key=hugging_face_api_key,
            )
        except WorkerError:
            raise
        except Exception as error:
            if _is_huggingface_auth_error(error):
                raise AssetAuthRequiredError("Hugging Face asset requires authentication.") from error
            raise AssetDownloadError("Hugging Face asset download failed.") from error


def _load_hub_url() -> HubUrl:
    try:
        from huggingface_hub import hf_hub_url
    except ImportError as error:
        raise AssetDownloadError("Hugging Face Hub client is unavailable.") from error
    return hf_hub_url


def _download_with_isolated_process(
    source: HuggingFaceSource,
    target: Path,
    hub_url: HubUrl | None,
    open_url: UrlOpen | None,
    *,
    download_inactivity_timeout_seconds: float | None,
    hugging_face_api_key: str | None,
) -> None:
    if download_inactivity_timeout_seconds is None:
        _download_to_target(source, target, hub_url, open_url, hugging_face_api_key)
        return

    context = get_context("spawn")
    result_queue = context.Queue()
    process = context.Process(
        target=_download_process,
        args=(
            source,
            str(target),
            hub_url,
            open_url,
            hugging_face_api_key,
            result_queue,
        ),
    )
    process.start()

    last_chunk_at = monotonic()
    while process.is_alive():
        last_chunk_at = _drain_chunk_events(result_queue, last_chunk_at)
        if monotonic() - last_chunk_at >= download_inactivity_timeout_seconds:
            _terminate_process(process)
            temporary = target.with_suffix(target.suffix + ".part")
            if temporary.exists():
                temporary.unlink()
            raise StepTimeoutError("Hugging Face asset download timed out due to inactivity.")
        process.join(timeout=0.1)

    _handle_download_process_result(result_queue)


def _drain_chunk_events(result_queue, last_chunk_at: float) -> float:
    while True:
        try:
            status, *payload = result_queue.get_nowait()
        except Empty:
            return last_chunk_at
        if status == "chunk":
            last_chunk_at = monotonic()
            continue
        result_queue.put((status, *payload))
        return last_chunk_at


def _handle_download_process_result(result_queue) -> None:
    while True:
        try:
            status, *payload = result_queue.get(timeout=1)
        except Empty as error:
            raise AssetDownloadError("Hugging Face asset download failed.") from error
        if status == "chunk":
            continue
        if status == "ok":
            return
        error_class, status_code, message = payload
        if error_class == "GatedRepoError" or status_code in (401, 403):
            raise AssetAuthRequiredError("Hugging Face asset requires authentication.")
        raise AssetDownloadError("Hugging Face asset download failed.") from RuntimeError(message)


def _download_to_target(
    source: HuggingFaceSource,
    target: Path,
    hub_url: HubUrl | None,
    open_url: UrlOpen | None,
    hugging_face_api_key: str | None,
    on_chunk: Callable[[], None] | None = None,
) -> None:
    temporary = target.with_suffix(target.suffix + ".part")
    url = _resolve_hub_url(source, hub_url)
    headers = {}
    if hugging_face_api_key:
        headers["Authorization"] = f"Bearer {hugging_face_api_key}"

    try:
        with (open_url or _open_request)(Request(url, headers=headers)) as response:
            with temporary.open("wb") as output:
                while True:
                    chunk = response.read(CHUNK_SIZE)
                    if not chunk:
                        break
                    output.write(chunk)
                    if on_chunk is not None:
                        on_chunk()
        temporary.replace(target)
    finally:
        if temporary.exists():
            temporary.unlink()


def _resolve_hub_url(source: HuggingFaceSource, hub_url: HubUrl | None) -> str:
    resolve_url = hub_url or _load_hub_url()
    return resolve_url(
        repo_id=source.repository_id,
        filename=source.file_path,
        revision=source.revision,
        repo_type="model",
    )


def _open_request(request: Request):
    return build_opener(_AuthorizationStrippingRedirectHandler()).open(request)


class _AuthorizationStrippingRedirectHandler(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        redirected = super().redirect_request(req, fp, code, msg, headers, newurl)
        if redirected is not None and _host(req.full_url) != _host(newurl):
            redirected.remove_header("Authorization")
        return redirected


def _host(url: str) -> str:
    return urlparse(url).netloc.lower()


def _download_process(
    source: HuggingFaceSource,
    target: str,
    hub_url: HubUrl | None,
    open_url: UrlOpen | None,
    hugging_face_api_key: str | None,
    result_queue,
) -> None:
    try:
        _download_to_target(
            source,
            Path(target),
            hub_url,
            open_url,
            hugging_face_api_key,
            on_chunk=lambda: result_queue.put(("chunk",)),
        )
        result_queue.put(("ok",))
    except BaseException as error:
        response = getattr(error, "response", None)
        status_code = getattr(response, "status_code", None) or getattr(
            error,
            "code",
            None,
        )
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
    if isinstance(error, HTTPError):
        return error.code in (401, 403)
    response = getattr(error, "response", None)
    status_code = getattr(response, "status_code", None)
    return status_code in (401, 403)
