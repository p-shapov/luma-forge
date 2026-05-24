import logging

from runpod_endpoint_worker.errors import EndpointWorkerError, safe_error_message


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


def _safe_log_message(message: str) -> str:
    sanitized = safe_error_message(message)
    if sanitized == "Endpoint worker request failed.":
        return "redacted"
    return sanitized
