import logging


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
    LOGGER.warning("%s: %s", message, _safe_log_message(str(error)))


def _safe_log_message(message: str) -> str:
    lowered = message.lower()
    unsafe_markers = ("secret", "token", "api key", "authorization", "bearer", "data:image", "base64")
    if any(marker in lowered for marker in unsafe_markers):
        return "redacted"
    return message
