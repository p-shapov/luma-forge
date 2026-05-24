from dataclasses import dataclass
from typing import Any

from runpod_endpoint_worker.config import EndpointConfig
from runpod_endpoint_worker.errors import UnsupportedExecutionTypeError, ValidationError


@dataclass(frozen=True)
class GenerationRequest:
    execution_type: str
    prompt: str
    job_id: str = "local"


@dataclass(frozen=True)
class GenerationImage:
    filename: str
    mime_type: str
    byte_size: int
    sha256: str
    artifact_uri: str
    storage_type: str
    relative_path: str

    def to_payload(self) -> dict[str, Any]:
        return {
            "filename": self.filename,
            "mime_type": self.mime_type,
            "byte_size": self.byte_size,
            "sha256": self.sha256,
            "artifact_uri": self.artifact_uri,
            "storage": {
                "type": self.storage_type,
                "relative_path": self.relative_path,
            },
        }


@dataclass(frozen=True)
class GenerationResponse:
    execution_type: str
    images: list[GenerationImage]

    def to_payload(self) -> dict[str, Any]:
        return {
            "status": "succeeded",
            "generation": {
                "implemented": True,
                "execution_type": self.execution_type,
                "images": [image.to_payload() for image in self.images],
            },
        }


def parse_generation_request(payload: Any, config: EndpointConfig, *, job_id: str = "local") -> GenerationRequest:
    data = _object(payload, "input")
    execution_type = _non_empty_string(data.get("execution_type"), "execution_type")
    if execution_type not in config.supported_execution_types:
        raise UnsupportedExecutionTypeError("Execution type is not supported.")

    prompt = _non_empty_string(data.get("prompt"), "prompt")
    if len(prompt) > config.max_prompt_chars:
        raise ValidationError("prompt is too large")

    return GenerationRequest(execution_type=execution_type, prompt=prompt, job_id=job_id)


def _object(payload: Any, field: str) -> dict[str, Any]:
    if not isinstance(payload, dict):
        raise ValidationError(f"{field} must be an object")
    return payload


def _non_empty_string(value: Any, field: str) -> str:
    if not isinstance(value, str) or value.strip() == "":
        raise ValidationError(f"{field} must be a non-empty string")
    return value.strip()
