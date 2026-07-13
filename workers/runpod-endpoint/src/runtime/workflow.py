from __future__ import annotations

import copy
import json
from pathlib import Path
from typing import Any

from app.errors import WorkflowValidationError


def load_workflow(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise WorkflowValidationError("Baked workflow could not be loaded.") from error
    if not isinstance(value, dict):
        raise WorkflowValidationError("Baked workflow must be a JSON object.")
    graph = value.get("graph")
    if not isinstance(graph, dict):
        raise WorkflowValidationError("Baked workflow must contain a graph object.")
    return graph


def load_execution_contract(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise WorkflowValidationError("Baked execution contract could not be loaded.") from error
    if not isinstance(value, dict):
        raise WorkflowValidationError("Baked execution contract must be a JSON object.")
    return value


def apply_input_bindings(
    workflow: dict[str, Any],
    execution_contract: dict[str, Any],
    inputs: dict[str, Any],
) -> dict[str, Any]:
    patched = copy.deepcopy(workflow)
    nodes = _nodes_by_id(patched)
    bindings = execution_contract.get("input_bindings")
    if not isinstance(bindings, list) or not bindings:
        raise WorkflowValidationError("Baked execution contract input bindings are invalid.")
    for binding in bindings:
        if not isinstance(binding, dict):
            raise WorkflowValidationError("Baked execution contract input binding is invalid.")
        node_id = binding.get("node_id")
        path = binding.get("path")
        if not isinstance(node_id, str) or node_id.strip() == "" or not isinstance(path, list) or not path:
            raise WorkflowValidationError("Baked execution contract input binding target is invalid.")
        node = nodes.get(node_id)
        if node is None:
            raise WorkflowValidationError("Baked workflow is missing an execution binding node.")
        value = _binding_value(binding.get("value"), inputs)
        _set_path_value(node, path, value)
    return patched


def write_patched_workflow(
    source_path: Path,
    execution_contract_path: Path,
    destination_path: Path,
    inputs: dict[str, Any],
) -> None:
    patched = apply_input_bindings(
        load_workflow(source_path),
        load_execution_contract(execution_contract_path),
        inputs,
    )
    destination_path.write_text(json.dumps(patched), encoding="utf-8")


def _binding_value(value: Any, inputs: dict[str, Any]) -> Any:
    if isinstance(value, str) and value.startswith("{{") and value.endswith("}}") and len(value) > 4:
        input_id = value[2:-2]
        if input_id not in inputs:
            raise WorkflowValidationError("Baked execution contract references a missing input.")
        return inputs[input_id]
    return copy.deepcopy(value)


def _set_path_value(target: dict[str, Any], path: list[Any], value: Any) -> None:
    current: Any = target
    for segment in path[:-1]:
        if not isinstance(segment, str):
            raise WorkflowValidationError("Baked execution contract path is invalid.")
        current = _path_child(current, segment)
    final = path[-1]
    if not isinstance(final, str):
        raise WorkflowValidationError("Baked execution contract path is invalid.")
    if isinstance(current, list):
        index = _array_index(final)
        if index >= len(current):
            raise WorkflowValidationError("Baked execution contract path is missing.")
        current[index] = value
        return
    if isinstance(current, dict):
        if final not in current:
            raise WorkflowValidationError("Baked execution contract path is missing.")
        current[final] = value
        return
    raise WorkflowValidationError("Baked execution contract path is invalid.")


def _path_child(current: Any, segment: str) -> Any:
    if isinstance(current, list):
        index = _array_index(segment)
        if index >= len(current):
            raise WorkflowValidationError("Baked execution contract path is missing.")
        return current[index]
    if isinstance(current, dict):
        if segment not in current:
            raise WorkflowValidationError("Baked execution contract path is missing.")
        return current[segment]
    raise WorkflowValidationError("Baked execution contract path is invalid.")


def _array_index(segment: str) -> int:
    if not segment.isdigit():
        raise WorkflowValidationError("Baked execution contract path is invalid.")
    return int(segment)


def _nodes_by_id(workflow: dict[str, Any]) -> dict[str, dict[str, Any]]:
    nodes = workflow.get("nodes")
    if not isinstance(nodes, list):
        raise WorkflowValidationError("Baked workflow must contain a nodes list.")

    indexed: dict[str, dict[str, Any]] = {}
    for node in nodes:
        if isinstance(node, dict) and "id" in node:
            indexed[str(node["id"])] = node
    return indexed
