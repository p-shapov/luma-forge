from __future__ import annotations

import copy
import json
from pathlib import Path
from typing import Any

from runpod_endpoint_worker.errors import WorkflowValidationError


EXPECTED_NODES = {
    "171": ("PrimitiveStringMultiline", "User Prompt"),
    "154": ("PrimitiveBoolean", "Switch to Image Edit"),
    "177": ("PrimitiveBoolean", "Enable Prompt Refine?"),
    "227": ("SaveImage", None),
}


def load_workflow(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise WorkflowValidationError("Baked workflow could not be loaded.") from error
    if not isinstance(value, dict):
        raise WorkflowValidationError("Baked workflow must be a JSON object.")
    return value


def validate_hidream_workflow(workflow: dict[str, Any]) -> None:
    nodes = _nodes_by_id(workflow)
    for node_id, (expected_type, expected_title) in EXPECTED_NODES.items():
        node = nodes.get(node_id)
        if node is None:
            raise WorkflowValidationError("Baked workflow is missing an expected HiDream node.")
        if node.get("type") != expected_type:
            raise WorkflowValidationError("Baked workflow contains an unexpected HiDream node type.")
        if expected_title is not None and node.get("title") != expected_title:
            raise WorkflowValidationError("Baked workflow contains an unexpected HiDream node title.")
        values = node.get("widgets_values")
        if not isinstance(values, list) or not values:
            raise WorkflowValidationError("Baked workflow contains an unexpected HiDream widget shape.")


def patch_hidream_workflow(workflow: dict[str, Any], prompt: str) -> dict[str, Any]:
    validate_hidream_workflow(workflow)
    patched = copy.deepcopy(workflow)
    nodes = _nodes_by_id(patched)
    nodes["171"]["widgets_values"][0] = prompt
    nodes["154"]["widgets_values"][0] = False
    nodes["177"]["widgets_values"][0] = False
    return patched


def write_patched_workflow(source_path: Path, destination_path: Path, prompt: str) -> None:
    patched = patch_hidream_workflow(load_workflow(source_path), prompt)
    destination_path.write_text(json.dumps(patched), encoding="utf-8")


def _nodes_by_id(workflow: dict[str, Any]) -> dict[str, dict[str, Any]]:
    nodes = workflow.get("nodes")
    if not isinstance(nodes, list):
        raise WorkflowValidationError("Baked workflow must contain a nodes list.")

    indexed: dict[str, dict[str, Any]] = {}
    for node in nodes:
        if isinstance(node, dict) and "id" in node:
            indexed[str(node["id"])] = node
    return indexed
