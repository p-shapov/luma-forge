WorkerErrorContextValue = str | int | float | bool | None
WorkerErrorContext = dict[str, WorkerErrorContextValue]
WorkerErrorPayload = dict[str, str | WorkerErrorContext]


class WorkerError(Exception):
    code = "worker_error"
    status = 400

    def __init__(
        self,
        message: str,
        *,
        code: str | None = None,
        context: WorkerErrorContext | None = None,
    ):
        super().__init__(message)
        self.message = message
        self.code = code or self.code
        self.context = context.copy() if context else None

    def to_dict(self) -> WorkerErrorPayload:
        payload: WorkerErrorPayload = {
            "code": self.code,
            "message": self.message,
        }
        if self.context:
            payload["context"] = self.context.copy()
        return payload


class ValidationError(WorkerError):
    code = "invalid_request"
    status = 400


class InvalidJsonError(WorkerError):
    code = "invalid_json"
    status = 400


class UnauthorizedError(WorkerError):
    code = "invalid_authorization"
    status = 401


class RequestTooLargeError(WorkerError):
    code = "request_body_too_large"
    status = 413


class ConflictError(WorkerError):
    code = "active_job_exists"
    status = 409


class NotFoundError(WorkerError):
    code = "endpoint_not_found"
    status = 404


class PreparationError(WorkerError):
    code = "preparation_failed"
    status = 500


class AssetDownloadError(PreparationError):
    code = "asset_download_failed"


class AssetAuthRequiredError(PreparationError):
    code = "asset_auth_required"


class PathValidationError(ValidationError):
    code = "path_validation_failed"


class StepTimeoutError(PreparationError):
    code = "step_timeout"
