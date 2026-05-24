import time
from typing import Any

from runpod_endpoint_worker.config import EndpointConfig
from runpod_endpoint_worker.errors import EndpointWorkerError, UnexpectedRuntimeError, safe_failure_payload
from runpod_endpoint_worker.logging import configure_logging, log_failure_context, log_unexpected_exception_context
from runpod_endpoint_worker.service import GenerationService


def create_handler(service: GenerationService):
    def handler(job: dict[str, Any]) -> dict[str, Any]:
        started = time.perf_counter()
        try:
            response = service.generate_from_payload(job.get("input"))
            return response.to_payload()
        except EndpointWorkerError as error:
            failure = safe_failure_payload(error)
            log_failure_context(
                "Endpoint worker request failed",
                job_id=_job_id(job),
                failure=failure,
                elapsed_ms=_elapsed_ms(started),
            )
            return {
                "status": "failed",
                "failure": failure,
                "error": _runpod_error_signal(failure),
            }
        except Exception as error:
            log_unexpected_exception_context(
                "Unexpected endpoint worker exception",
                job_id=_job_id(job),
                error=error,
            )
            wrapped = UnexpectedRuntimeError("Endpoint worker runtime failed.")
            failure = safe_failure_payload(wrapped)
            log_failure_context(
                "Unexpected endpoint worker failure",
                job_id=_job_id(job),
                failure=failure,
                elapsed_ms=_elapsed_ms(started),
            )
            return {
                "status": "failed",
                "failure": failure,
                "error": _runpod_error_signal(failure),
            }

    return handler


def _job_id(job: dict[str, Any]) -> str | None:
    job_id = job.get("id")
    return job_id if isinstance(job_id, str) and job_id.strip() != "" else None


def _elapsed_ms(started: float) -> int:
    return max(0, int((time.perf_counter() - started) * 1000))


def _runpod_error_signal(failure: dict[str, Any]) -> str:
    return f"{failure['code']}: {failure['message']}"


def build_default_handler():
    config = EndpointConfig.from_env()
    return create_handler(GenerationService.from_config(config))


def start_runpod_worker() -> None:
    configure_logging()
    try:
        import runpod
    except ImportError as error:
        raise RuntimeError("runpod package is required to start the endpoint worker") from error

    runpod.serverless.start({"handler": build_default_handler()})
