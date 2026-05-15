import json
import hmac
from collections.abc import Callable
from http.server import BaseHTTPRequestHandler
from typing import Any

from app.config import WorkerConfig
from app.errors import (
    InvalidJsonError,
    NotFoundError,
    RequestTooLargeError,
    UnauthorizedError,
    ValidationError,
    WorkerError,
)
from orchestration.preparation_job import JobManager
from app.schemas import parse_cancel_request, parse_start_request


class ProvisionerRequestHandler(BaseHTTPRequestHandler):
    manager: JobManager | None = None
    config: WorkerConfig | None = None

    def do_GET(self) -> None:
        self._handle_request(
            {
                "/status": self._handle_status,
            },
            read_json=False,
            success_status=200,
        )

    def do_POST(self) -> None:
        self._handle_request(
            {
                "/start": self._handle_start,
                "/cancel": self._handle_cancel,
            },
            read_json=True,
            success_status=202,
        )

    def log_message(self, format: str, *args: Any) -> None:
        return

    def send_error(self, code: int, message: str | None = None, explain: str | None = None) -> None:
        if code == 501:
            self._handle_unsupported_method()
            return
        super().send_error(code, message, explain)

    def _handle_request(
        self,
        routes: dict[str, Callable[[Any], dict[str, Any]]],
        *,
        read_json: bool,
        success_status: int,
    ) -> None:
        try:
            self._authorize()
            handler = routes.get(self.path)
            if handler is None:
                raise NotFoundError("Endpoint not found")
            payload = self._read_json() if read_json else None
            self._json(success_status, handler(payload))
        except WorkerError as error:
            self._worker_error(error)
        except json.JSONDecodeError:
            self._worker_error(InvalidJsonError("Request body must be valid JSON."))

    def _handle_unsupported_method(self) -> None:
        try:
            self._authorize()
            raise NotFoundError("Endpoint not found")
        except WorkerError as error:
            self._worker_error(error)

    def _handle_status(self, payload: Any) -> dict[str, Any]:
        return self._manager().status().to_dict()

    def _handle_start(self, payload: Any) -> dict[str, Any]:
        return self._manager().start(parse_start_request(payload)).to_dict()

    def _handle_cancel(self, payload: Any) -> dict[str, Any]:
        return self._manager().cancel(parse_cancel_request(payload)).to_dict()

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
        if authorization is None:
            raise UnauthorizedError("Unauthorized.")
        try:
            authorization_bytes = authorization.encode("ascii")
            expected_authorization = f"Bearer {token}".encode("ascii")
        except UnicodeEncodeError as error:
            raise UnauthorizedError("Unauthorized.") from error
        if not hmac.compare_digest(authorization_bytes, expected_authorization):
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
