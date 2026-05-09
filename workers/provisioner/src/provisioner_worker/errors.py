class WorkerError(Exception):
    code = "worker_error"
    status = 400

    def __init__(self, message: str):
        super().__init__(message)
        self.message = message


class ValidationError(WorkerError):
    code = "invalid_request"
    status = 400


class ConflictError(WorkerError):
    code = "job_already_running"
    status = 409


class NotFoundError(WorkerError):
    code = "not_found"
    status = 404


class PreparationError(WorkerError):
    code = "preparation_failed"
    status = 500

