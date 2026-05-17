from dataclasses import dataclass
from typing import Any

from runpod_endpoint_worker.comfyui import ComfyUiClient, ComfyUiProcessManager, render_t2i_workflow
from runpod_endpoint_worker.config import EndpointConfig
from runpod_endpoint_worker.environment import validate_prepared_environment
from runpod_endpoint_worker.schemas import GenerationResponse, parse_generation_request


@dataclass
class GenerationService:
    config: EndpointConfig
    comfyui: ComfyUiClient
    process_manager: ComfyUiProcessManager | None = None

    @classmethod
    def from_config(cls, config: EndpointConfig) -> "GenerationService":
        comfyui = ComfyUiClient(
            base_url=config.comfyui_base_url,
            timeout_seconds=config.generation_timeout_seconds,
        )
        return cls(
            config=config,
            comfyui=comfyui,
            process_manager=ComfyUiProcessManager(config=config, client=comfyui),
        )

    def generate_from_payload(self, payload: Any) -> GenerationResponse:
        request = parse_generation_request(payload, self.config)
        runtime = validate_prepared_environment(self.config)
        if self.process_manager is not None:
            self.process_manager.ensure_running(runtime)
        else:
            self.comfyui.assert_available()
        workflow = render_t2i_workflow(self.config, request, runtime)
        prompt_id = self.comfyui.queue_prompt(workflow)
        return GenerationResponse(image=self.comfyui.wait_for_image(prompt_id))
