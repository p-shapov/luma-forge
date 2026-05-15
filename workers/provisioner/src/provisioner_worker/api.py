import json
import hmac
from http.server import BaseHTTPRequestHandler
from typing import Any

from provisioner_worker.config import WorkerConfig
from provisioner_worker.errors import (
    InvalidJsonError,
    NotFoundError,
    RequestTooLargeError,
    UnauthorizedError,
    ValidationError,
    WorkerError,
)
from provisioner_worker.job_manager import JobManager
from provisioner_worker.schemas import parse_cancel_request, parse_start_request


class ProvisionerRequestHandler(BaseHTTPRequestHandler):
    manager: JobManager | None = None
    config: WorkerConfig | None = None

    def do_GET(self) -> None:
        try:
            self._authorize()
            if self.path != "/status":
                raise NotFoundError("Endpoint not found")
            self._json(200, self._manager().status().to_dict())
        except WorkerError as error:
            self._worker_error(error)

    def do_POST(self) -> None:
        try:
            self._authorize()
            payload = self._read_json()
            if self.path == "/start":
                snapshot = self._manager().start(parse_start_request(payload))
                self._json(202, snapshot.to_dict())
            elif self.path == "/cancel":
                snapshot = self._manager().cancel(parse_cancel_request(payload))
                self._json(202, snapshot.to_dict())
            else:
                raise NotFoundError("Endpoint not found")
        except WorkerError as error:
            self._worker_error(error)
        except json.JSONDecodeError:
            self._worker_error(InvalidJsonError("Request body must be valid JSON."))

    def log_message(self, format: str, *args: Any) -> None:
        return

    def _read_json(self) -> Any:
        raw_content_length = self.headers.get("Content-Length")
        if raw_content_length is None:
            raise ValidationError("Content-Length header is required.", reason_code="missing_content_length")
        try:
            content_length = int(raw_content_length)
        except ValueError as error:
            raise ValidationError(
                "Content-Length header must be an integer.",
                reason_code="malformed_content_length",
            ) from error
        if content_length < 0:
            raise ValidationError(
                "Content-Length header must be non-negative.",
                reason_code="negative_content_length",
            )
        if content_length > self._config().max_request_bytes:
            raise RequestTooLargeError(
                "Request body is too large.",
                context={"max_request_bytes": self._config().max_request_bytes},
            )
        if content_length == 0:
            return {}
        raw = self.rfile.read(content_length)
        return json.loads(raw.decode("utf-8"))

    def _authorize(self) -> None:
        token = self._config().bearer_token
        authorization = self.headers.get("Authorization")
        if authorization is None or not hmac.compare_digest(authorization, f"Bearer {token}"):
            raise UnauthorizedError("Unauthorized.")

    def _worker_error(self, error: WorkerError) -> None:
        self._json(error.status, error.to_dict())

    def _json(self, status: int, payload: dict[str, Any]) -> None:
        body = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _config(self) -> WorkerConfig:
        if self.config is None:
            raise RuntimeError("Provisioner request handler is missing runtime config.")
        return self.config

    def _manager(self) -> JobManager:
        if self.manager is None:
            raise RuntimeError("Provisioner request handler is missing job manager.")
        return self.manager
