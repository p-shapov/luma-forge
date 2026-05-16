from typing import Any

from runpod_endpoint_worker.config import EndpointConfig
from runpod_endpoint_worker.errors import EndpointWorkerError, UnexpectedRuntimeError, safe_error_payload
from runpod_endpoint_worker.logging import configure_logging, log_safe_error
from runpod_endpoint_worker.service import GenerationService


def create_handler(service: GenerationService):
    def handler(job: dict[str, Any]) -> dict[str, Any]:
        try:
            response = service.generate_from_payload(job.get("input"))
            return response.to_payload()
        except EndpointWorkerError as error:
            log_safe_error("Endpoint worker request failed", error)
            return {
                "status": "failed",
                "error": safe_error_payload(error),
            }
        except Exception as error:
            wrapped = UnexpectedRuntimeError("Endpoint worker runtime failed.")
            log_safe_error("Unexpected endpoint worker failure", error)
            return {
                "status": "failed",
                "error": safe_error_payload(wrapped),
            }

    return handler


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
