class WorkerError(Exception):
    code = "worker_error"
    status = 400

    def __init__(self, message: str):
        super().__init__(message)
        self.message = message


class ValidationError(WorkerError):
    code = "invalid_request"
    status = 400


class UnauthorizedError(WorkerError):
    code = "unauthorized"
    status = 401


class RequestTooLargeError(WorkerError):
    code = "request_too_large"
    status = 413


class ConflictError(WorkerError):
    code = "job_already_running"
    status = 409


class NotFoundError(WorkerError):
    code = "not_found"
    status = 404


class PreparationError(WorkerError):
    code = "preparation_failed"
    status = 500


class GitCheckoutError(PreparationError):
    code = "git_checkout_failed"


class DependencyInstallError(PreparationError):
    code = "dependency_install_failed"


class AssetDownloadError(PreparationError):
    code = "asset_download_failed"


class AssetAuthRequiredError(PreparationError):
    code = "asset_auth_required"


class PathValidationError(ValidationError):
    code = "path_validation_failed"


class StepTimeoutError(PreparationError):
    code = "step_timeout"
