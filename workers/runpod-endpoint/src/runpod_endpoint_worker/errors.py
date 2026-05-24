class EndpointWorkerError(Exception):
    code = "runpod_endpoint_worker_error"

    def __init__(self, message: str):
        super().__init__(message)
        self.message = message


class ValidationError(EndpointWorkerError):
    code = "invalid_request"


class UnsupportedExecutionTypeError(ValidationError):
    code = "unsupported_execution_type"


class WorkflowValidationError(EndpointWorkerError):
    code = "workflow_validation_failed"


class ComfyStartupError(EndpointWorkerError):
    code = "comfyui_startup_failed"


class ComfyLaunchError(ComfyStartupError):
    code = "comfyui_launch_failed"


class ComfyStartupTimeoutError(ComfyStartupError):
    code = "comfyui_startup_timeout"


class ComfyExecutionError(EndpointWorkerError):
    code = "comfyui_execution_failed"


class ComfyWorkflowError(ComfyExecutionError):
    code = "comfyui_workflow_failed"


class ComfyWorkflowTimeoutError(ComfyExecutionError):
    code = "comfyui_workflow_timeout"


class ComfyOutputParseError(ComfyExecutionError):
    code = "comfyui_output_parse_failed"


class ComfyNoOutputsError(ComfyExecutionError):
    code = "comfyui_no_outputs"


class ComfyOutputFetchError(ComfyExecutionError):
    code = "comfyui_output_fetch_failed"


class ResponseTooLargeError(ComfyExecutionError):
    code = "response_too_large"


class UnexpectedRuntimeError(EndpointWorkerError):
    code = "runtime_failed"


def safe_error_payload(error: EndpointWorkerError) -> dict[str, str]:
    return {
        "code": error.code,
        "message": safe_error_message(error.message),
    }


def safe_error_message(message: str) -> str:
    normalized = " ".join(message.split())
    unsafe_markers = (
        "secret",
        "token",
        "api key",
        "authorization",
        "bearer",
        "password",
        "credential",
        "data:image",
        "base64",
    )
    lowered = normalized.lower()
    if any(marker in lowered for marker in unsafe_markers):
        return "Endpoint worker request failed."
    if len(normalized) > 600:
        return f"{normalized[:597]}..."
    return normalized
