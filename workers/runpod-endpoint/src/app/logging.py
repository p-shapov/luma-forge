import logging
from typing import Any

from app.errors import EndpointWorkerError, safe_error_message


LOGGER = logging.getLogger("runpod_endpoint_worker")
LOGGER.addHandler(logging.NullHandler())
LOGGER.propagate = False


def configure_logging() -> None:
    if not any(not isinstance(handler, logging.NullHandler) for handler in LOGGER.handlers):
        handler = logging.StreamHandler()
        handler.setFormatter(logging.Formatter("%(levelname)s %(name)s: %(message)s"))
        LOGGER.addHandler(handler)
    LOGGER.setLevel(logging.INFO)


def log_safe_error(message: str, error: Exception) -> None:
    if isinstance(error, EndpointWorkerError):
        LOGGER.warning("%s: %s: %s", message, error.code, safe_error_message(error.message))
        return
    LOGGER.warning("%s: %s", message, _safe_log_message(str(error)))


def log_failure_context(
    message: str,
    *,
    job_id: str | None,
    failure: dict[str, Any],
    elapsed_ms: int,
) -> None:
    LOGGER.warning(
        "%s job_id=%s code=%s stage=%s retryable=%s elapsed_ms=%d message=%s metadata=%s",
        message,
        job_id or "unknown",
        failure["code"],
        failure["stage"],
        failure["retryable"],
        elapsed_ms,
        failure["message"],
        failure.get("metadata", {}),
    )


def log_unexpected_exception_context(message: str, *, job_id: str | None, error: Exception) -> None:
    LOGGER.warning(
        "%s job_id=%s exception_type=%s exception_message=%s",
        message,
        job_id or "unknown",
        type(error).__name__,
        _safe_log_message(str(error)),
    )


def _safe_log_message(message: str) -> str:
    sanitized = safe_error_message(message)
    if sanitized == "Endpoint worker request failed.":
        return "redacted"
    return sanitized
