from dataclasses import dataclass
from typing import Any

from runpod_endpoint_worker.config import EndpointConfig
from runpod_endpoint_worker.errors import ValidationError


@dataclass(frozen=True)
class GenerationRequest:
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
    images: list[GenerationImage]

    def to_payload(self) -> dict[str, Any]:
        return {
            "status": "succeeded",
            "generation": {
                "implemented": True,
                "images": [image.to_payload() for image in self.images],
            },
        }


def parse_generation_request(payload: Any, config: EndpointConfig, *, job_id: str = "local") -> GenerationRequest:
    data = _object(payload, "input")
    prompt = _non_empty_string(data.get("prompt"), "prompt")
    if len(prompt) > config.max_prompt_chars:
        raise ValidationError("prompt is too large")

    return GenerationRequest(prompt=prompt, job_id=job_id)


def _object(payload: Any, field: str) -> dict[str, Any]:
    if not isinstance(payload, dict):
        raise ValidationError(f"{field} must be an object")
    return payload


def _non_empty_string(value: Any, field: str) -> str:
    if not isinstance(value, str) or value.strip() == "":
        raise ValidationError(f"{field} must be a non-empty string")
    return value.strip()
