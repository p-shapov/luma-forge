class EndpointWorkerError(Exception):
    code = "runpod_endpoint_worker_error"

    def __init__(self, message: str):
        super().__init__(message)
        self.message = message


class ValidationError(EndpointWorkerError):
    code = "invalid_request"


class UnsupportedExecutionTypeError(ValidationError):
    code = "unsupported_execution_type"


class UnexpectedRuntimeError(EndpointWorkerError):
    code = "runtime_failed"


def safe_error_payload(error: EndpointWorkerError) -> dict[str, str]:
    return {
        "code": error.code,
        "message": _safe_message(error.message),
    }


def _safe_message(message: str) -> str:
    unsafe_markers = ("secret", "token", "api key", "authorization", "bearer")
    lowered = message.lower()
    if any(marker in lowered for marker in unsafe_markers):
        return "Endpoint worker request failed."
    return message
