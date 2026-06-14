class EndpointWorkerError(Exception):
    code = "runpod_endpoint_worker_error"
    stage = "runtime"
    retryable = False

    def __init__(self, message: str, metadata: dict[str, object] | None = None):
        super().__init__(message)
        self.message = message
        self.metadata = safe_failure_metadata(metadata or {})


class ValidationError(EndpointWorkerError):
    code = "invalid_request"
    stage = "request_validation"


class WorkflowValidationError(EndpointWorkerError):
    code = "workflow_validation_failed"
    stage = "workflow_validation"


class ComfyStartupError(EndpointWorkerError):
    code = "comfyui_startup_failed"
    stage = "comfyui_startup"
    retryable = True


class ComfyLaunchError(ComfyStartupError):
    code = "comfyui_launch_failed"
    stage = "comfyui_launch"
    retryable = False


class ComfyStartupTimeoutError(ComfyStartupError):
    code = "comfyui_startup_timeout"


class ComfyExecutionError(EndpointWorkerError):
    code = "comfyui_execution_failed"
    stage = "workflow_execution"


class ComfyWorkflowError(ComfyExecutionError):
    code = "comfyui_workflow_failed"


class ComfyWorkflowTimeoutError(ComfyExecutionError):
    code = "comfyui_workflow_timeout"
    retryable = True


class ComfyOutputParseError(ComfyExecutionError):
    code = "comfyui_output_parse_failed"
    stage = "output_parse"


class ComfyNoOutputsError(ComfyExecutionError):
    code = "comfyui_no_outputs"
    stage = "output_parse"


class ComfyOutputFetchError(ComfyExecutionError):
    code = "comfyui_output_fetch_failed"
    stage = "output_fetch"
    retryable = True


class ResponseTooLargeError(ComfyExecutionError):
    code = "response_too_large"
    stage = "response_size"


class UnexpectedRuntimeError(EndpointWorkerError):
    code = "runtime_failed"
    retryable = True


def safe_failure_payload(error: EndpointWorkerError) -> dict[str, object]:
    payload: dict[str, object] = {
        "code": error.code,
        "message": safe_error_message(error.message),
        "stage": error.stage,
        "retryable": error.retryable,
    }
    if error.metadata:
        payload["metadata"] = error.metadata
    return payload


def safe_failure_metadata(metadata: dict[str, object]) -> dict[str, object]:
    safe: dict[str, object] = {}
    for key in (
        "exit_status",
        "timeout_seconds",
        "diagnostic_excerpt",
        "comfy_error_kind",
        "comfy_error_message",
        "comfy_node_id",
        "comfy_class_type",
        "comfy_exception_type",
        "comfy_status_code",
        "comfy_node_errors",
        "missing_model_paths",
    ):
        value = metadata.get(key)
        if isinstance(value, bool):
            continue
        if isinstance(value, int | float | str):
            safe[key] = safe_error_message(value) if isinstance(value, str) else value
        if isinstance(value, list) and all(isinstance(item, str) for item in value):
            safe[key] = [safe_error_message(item) for item in value]
    return safe


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
