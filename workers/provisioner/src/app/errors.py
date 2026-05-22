WorkerErrorContextValue = str | int | float | bool | None
WorkerErrorContext = dict[str, WorkerErrorContextValue]
WorkerErrorPayload = dict[str, str | WorkerErrorContext]


class WorkerError(Exception):
    code = "worker_error"
    reason_code = "worker_error"
    status = 400

    def __init__(
        self,
        message: str,
        *,
        reason_code: str | None = None,
        context: WorkerErrorContext | None = None,
    ):
        super().__init__(message)
        self.message = message
        self.reason_code = reason_code or self.reason_code
        self.context = context.copy() if context else None

    def to_dict(self) -> WorkerErrorPayload:
        payload: WorkerErrorPayload = {
            "code": self.code,
            "reason_code": self.reason_code,
            "message": self.message,
        }
        if self.context:
            payload["context"] = self.context.copy()
        return payload


class ValidationError(WorkerError):
    code = "invalid_request"
    reason_code = "invalid_request"
    status = 400


class InvalidJsonError(WorkerError):
    code = "invalid_json"
    reason_code = "invalid_json"
    status = 400


class UnauthorizedError(WorkerError):
    code = "unauthorized"
    reason_code = "invalid_authorization"
    status = 401


class RequestTooLargeError(WorkerError):
    code = "request_too_large"
    reason_code = "request_body_too_large"
    status = 413


class ConflictError(WorkerError):
    code = "job_already_running"
    reason_code = "active_job_exists"
    status = 409


class NotFoundError(WorkerError):
    code = "not_found"
    reason_code = "endpoint_not_found"
    status = 404


class PreparationError(WorkerError):
    code = "preparation_failed"
    reason_code = "preparation_failed"
    status = 500


class AssetDownloadError(PreparationError):
    code = "asset_download_failed"
    reason_code = "asset_download_failed"


class AssetAuthRequiredError(PreparationError):
    code = "asset_auth_required"
    reason_code = "asset_auth_required"


class PathValidationError(ValidationError):
    code = "path_validation_failed"
    reason_code = "path_validation_failed"


class StepTimeoutError(PreparationError):
    code = "step_timeout"
    reason_code = "step_timeout"
