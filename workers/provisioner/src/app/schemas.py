from dataclasses import dataclass
from pathlib import Path
import re
from typing import Any

from app.errors import ValidationError
from auxiliary.paths import safe_custom_node_relative_path, safe_relative_path

MAX_IDENTIFIER_LENGTH = 128
MAX_DISPLAY_NAME_LENGTH = 256
SAFE_IDENTIFIER_PATTERN = re.compile(r"[A-Za-z0-9._-]+")


@dataclass(frozen=True)
class GitSource:
    repository_url: str
    revision: str


@dataclass(frozen=True)
class HuggingFaceSource:
    repository_id: str
    file_path: str
    revision: str


@dataclass(frozen=True)
class ModelAssetInstall:
    comfyui_relative_path: Path


@dataclass(frozen=True)
class ModelAsset:
    id: str
    name: str
    download_source: HuggingFaceSource
    install: ModelAssetInstall


@dataclass(frozen=True)
class CustomNodeInstall:
    comfyui_custom_nodes_relative_path: Path
    python_requirements_path: Path | None


@dataclass(frozen=True)
class CustomNode:
    id: str
    name: str
    git_source: GitSource
    install: CustomNodeInstall


@dataclass(frozen=True)
class WorkflowPreset:
    id: str
    version: str
    name: str
    required_runtime_contract: "RuntimeContractReference"
    required_model_assets: list[ModelAsset]
    required_custom_nodes: list[CustomNode]


@dataclass(frozen=True)
class StartRequest:
    job_id: str
    workflow_preset: WorkflowPreset
    resolved_runtime_implementation: "ResolvedRuntimeImplementation"


@dataclass(frozen=True)
class RuntimeContractReference:
    id: str
    version: str


@dataclass(frozen=True)
class RuntimeMetadata:
    environment_kind: str
    python_version: str
    platform: str
    comfyui_revision: str
    runtime_manifest_compatibility: dict[str, str]
    workspace_overlay_policy: "WorkspaceOverlayPolicy"


@dataclass(frozen=True)
class WorkspaceOverlayPolicy:
    python_overlay_path: Path
    import_path_precedence: str
    protected_package_names: list[str]
    protected_package_prefixes: list[str]


@dataclass(frozen=True)
class RuntimeImageMetadata:
    image_runtime_root_path: Path
    image_python_interpreter_path: Path
    image_comfyui_root_path: Path
    image_base_dependency_record_paths: list[Path]
    provisioner_runtime_metadata_path: Path
    endpoint_runtime_contract_path: Path


@dataclass(frozen=True)
class ResolvedRuntimeImplementation:
    contract_id: str
    contract_version: str
    implementation_revision: str
    provisioner_image_ref: str
    endpoint_image_ref: str
    runtime_metadata: RuntimeMetadata
    image_metadata: RuntimeImageMetadata


def parse_start_request(payload: Any) -> StartRequest:
    data = _object(payload, "request")
    job_id = _non_empty_string(data.get("job_id"), "job_id")
    workflow_preset = _parse_workflow_preset(data.get("workflow_preset"))
    return StartRequest(
        job_id=job_id,
        workflow_preset=workflow_preset,
        resolved_runtime_implementation=_parse_resolved_runtime_implementation(
            data.get("resolved_runtime_implementation"),
        ),
    )


def _parse_workflow_preset(payload: Any) -> WorkflowPreset:
    data = _object(payload, "workflow_preset")
    return WorkflowPreset(
        id=_safe_identifier(data.get("id"), "workflow_preset.id"),
        version=_non_empty_string(data.get("version"), "workflow_preset.version"),
        name=_display_name(data.get("name"), "workflow_preset.name"),
        required_runtime_contract=_parse_runtime_contract_reference(
            data.get("required_runtime_contract"),
            "workflow_preset.required_runtime_contract",
        ),
        required_model_assets=[
            _parse_model_asset(item, f"workflow_preset.required_model_assets[{index}]")
            for index, item in enumerate(_list(data.get("required_model_assets"), "workflow_preset.required_model_assets"))
        ],
        required_custom_nodes=[
            _parse_custom_node(item, f"workflow_preset.required_custom_nodes[{index}]")
            for index, item in enumerate(_list(data.get("required_custom_nodes"), "workflow_preset.required_custom_nodes"))
        ],
    )


def _parse_runtime_contract_reference(payload: Any, field: str) -> RuntimeContractReference:
    data = _object(payload, field)
    return RuntimeContractReference(
        id=_safe_identifier(data.get("id"), f"{field}.id"),
        version=_non_empty_string(data.get("version"), f"{field}.version"),
    )


def _parse_resolved_runtime_implementation(payload: Any) -> ResolvedRuntimeImplementation:
    data = _object(payload, "resolved_runtime_implementation")
    runtime_metadata = _object(data.get("runtime_metadata"), "resolved_runtime_implementation.runtime_metadata")
    image_metadata = _object(data.get("image_metadata"), "resolved_runtime_implementation.image_metadata")
    return ResolvedRuntimeImplementation(
        contract_id=_safe_identifier(data.get("contract_id"), "resolved_runtime_implementation.contract_id"),
        contract_version=_non_empty_string(data.get("contract_version"), "resolved_runtime_implementation.contract_version"),
        implementation_revision=_non_empty_string(
            data.get("implementation_revision"),
            "resolved_runtime_implementation.implementation_revision",
        ),
        provisioner_image_ref=_immutable_image_ref(
            data.get("provisioner_image_ref"),
            "resolved_runtime_implementation.provisioner_image_ref",
        ),
        endpoint_image_ref=_immutable_image_ref(
            data.get("endpoint_image_ref"),
            "resolved_runtime_implementation.endpoint_image_ref",
        ),
        runtime_metadata=RuntimeMetadata(
            environment_kind=_non_empty_string(runtime_metadata.get("environment_kind"), "runtime_metadata.environment_kind"),
            python_version=_non_empty_string(runtime_metadata.get("python_version"), "runtime_metadata.python_version"),
            platform=_non_empty_string(runtime_metadata.get("platform"), "runtime_metadata.platform"),
            comfyui_revision=_immutable_revision(runtime_metadata.get("comfyui_revision"), "runtime_metadata.comfyui_revision"),
            runtime_manifest_compatibility=_string_map(
                runtime_metadata.get("runtime_manifest_compatibility"),
                "runtime_metadata.runtime_manifest_compatibility",
            ),
            workspace_overlay_policy=_parse_workspace_overlay_policy(
                runtime_metadata.get("workspace_overlay_policy"),
            ),
        ),
        image_metadata=RuntimeImageMetadata(
            image_runtime_root_path=_absolute_path(
                image_metadata.get("image_runtime_root_path"),
                "image_metadata.image_runtime_root_path",
            ),
            image_python_interpreter_path=_absolute_path(
                image_metadata.get("image_python_interpreter_path"),
                "image_metadata.image_python_interpreter_path",
            ),
            image_comfyui_root_path=_absolute_path(
                image_metadata.get("image_comfyui_root_path"),
                "image_metadata.image_comfyui_root_path",
            ),
            image_base_dependency_record_paths=[
                safe_relative_path(path, field_name="image_metadata.image_base_dependency_record_paths[]")
                for path in _list(
                    image_metadata.get("image_base_dependency_record_paths"),
                    "image_metadata.image_base_dependency_record_paths",
                )
            ],
            provisioner_runtime_metadata_path=_absolute_path(
                image_metadata.get("provisioner_runtime_metadata_path"),
                "image_metadata.provisioner_runtime_metadata_path",
            ),
            endpoint_runtime_contract_path=_absolute_path(
                image_metadata.get("endpoint_runtime_contract_path"),
                "image_metadata.endpoint_runtime_contract_path",
            ),
        ),
    )


def _parse_workspace_overlay_policy(payload: Any) -> WorkspaceOverlayPolicy:
    data = _object(payload, "runtime_metadata.workspace_overlay_policy")
    return WorkspaceOverlayPolicy(
        python_overlay_path=safe_relative_path(
            data.get("python_overlay_path"),
            field_name="runtime_metadata.workspace_overlay_policy.python_overlay_path",
        ),
        import_path_precedence=_non_empty_string(
            data.get("import_path_precedence"),
            "runtime_metadata.workspace_overlay_policy.import_path_precedence",
        ),
        protected_package_names=[
            _package_name(name, "runtime_metadata.workspace_overlay_policy.protected_package_names[]")
            for name in _list(
                data.get("protected_package_names"),
                "runtime_metadata.workspace_overlay_policy.protected_package_names",
            )
        ],
        protected_package_prefixes=[
            _package_prefix(prefix, "runtime_metadata.workspace_overlay_policy.protected_package_prefixes[]")
            for prefix in _list(
                data.get("protected_package_prefixes"),
                "runtime_metadata.workspace_overlay_policy.protected_package_prefixes",
            )
        ],
    )


def _parse_git_source(payload: Any, field: str) -> GitSource:
    data = _object(payload, field)
    source_type = _non_empty_string(data.get("source_type"), f"{field}.source_type")
    if source_type != "git":
        raise ValidationError(f"{field}.source_type must be git")
    revision = _non_empty_string(data.get("revision"), f"{field}.revision")
    if not _is_immutable_git_revision(revision):
        raise ValidationError(f"{field}.revision must be a full immutable commit hash")
    return GitSource(
        repository_url=_non_empty_string(data.get("repository_url"), f"{field}.repository_url"),
        revision=revision,
    )


def _parse_huggingface_source(payload: Any, field: str) -> HuggingFaceSource:
    data = _object(payload, field)
    source_type = _non_empty_string(data.get("source_type"), f"{field}.source_type")
    if source_type != "huggingface":
        raise ValidationError(f"{field}.source_type must be huggingface")
    return HuggingFaceSource(
        repository_id=_non_empty_string(data.get("repository_id"), f"{field}.repository_id"),
        file_path=safe_relative_path(data.get("file_path"), field_name=f"{field}.file_path").as_posix(),
        revision=_non_empty_string(data.get("revision"), f"{field}.revision"),
    )


def _parse_model_asset(payload: Any, field: str) -> ModelAsset:
    data = _object(payload, field)
    install = _object(data.get("install"), f"{field}.install")
    return ModelAsset(
        id=_safe_identifier(data.get("id"), f"{field}.id"),
        name=_display_name(data.get("name"), f"{field}.name"),
        download_source=_parse_huggingface_source(data.get("download_source"), f"{field}.download_source"),
        install=ModelAssetInstall(
            comfyui_relative_path=safe_relative_path(
                install.get("comfyui_relative_path"),
                field_name=f"{field}.install.comfyui_relative_path",
            ),
        ),
    )


def _parse_custom_node(payload: Any, field: str) -> CustomNode:
    data = _object(payload, field)
    install = _object(data.get("install"), f"{field}.install")
    requirements = install.get("python_requirements_path")
    return CustomNode(
        id=_safe_identifier(data.get("id"), f"{field}.id"),
        name=_display_name(data.get("name"), f"{field}.name"),
        git_source=_parse_git_source(data.get("git_source"), f"{field}.git_source"),
        install=CustomNodeInstall(
            comfyui_custom_nodes_relative_path=safe_custom_node_relative_path(
                install.get("comfyui_custom_nodes_relative_path"),
                field_name=f"{field}.install.comfyui_custom_nodes_relative_path",
            ),
            python_requirements_path=None
            if requirements is None
            else safe_relative_path(
                requirements,
                field_name=f"{field}.install.python_requirements_path",
            ),
        ),
    )


def _object(payload: Any, field: str) -> dict[str, Any]:
    if not isinstance(payload, dict):
        raise ValidationError(f"{field} must be an object")
    return payload


def _list(payload: Any, field: str) -> list[Any]:
    if not isinstance(payload, list):
        raise ValidationError(f"{field} must be an array")
    return payload


def _string_map(payload: Any, field: str) -> dict[str, str]:
    data = _object(payload, field)
    if any(not isinstance(key, str) or not isinstance(value, str) or value.strip() == "" for key, value in data.items()):
        raise ValidationError(f"{field} must contain string values")
    return {key: value.strip() for key, value in data.items()}


def _non_empty_string(value: Any, field: str) -> str:
    if not isinstance(value, str) or value.strip() == "":
        raise ValidationError(f"{field} must be a non-empty string")
    return value.strip()


def _safe_identifier(value: Any, field: str) -> str:
    if not isinstance(value, str) or value == "":
        raise ValidationError(f"{field} must be a non-empty string")
    identifier = value
    if len(identifier) > MAX_IDENTIFIER_LENGTH or SAFE_IDENTIFIER_PATTERN.fullmatch(identifier) is None:
        raise ValidationError(f"{field} must be a safe identifier")
    return identifier


def _display_name(value: Any, field: str) -> str:
    name = _non_empty_string(value, field)
    if len(name) > MAX_DISPLAY_NAME_LENGTH or any(ord(character) < 32 or ord(character) == 127 for character in name):
        raise ValidationError(f"{field} must be a safe display name")
    return name


def _package_name(value: Any, field: str) -> str:
    package = _non_empty_string(value, field).lower().replace("_", "-")
    if re.fullmatch(r"[a-z0-9][a-z0-9._-]*", package) is None:
        raise ValidationError(f"{field} must be a package name")
    return package


def _package_prefix(value: Any, field: str) -> str:
    prefix = _package_name(value, field)
    if not prefix.endswith("-"):
        raise ValidationError(f"{field} must end with '-'")
    return prefix


def _is_immutable_git_revision(value: str) -> bool:
    return re.fullmatch(r"[0-9a-f]{40}", value) is not None


def _immutable_revision(value: Any, field: str) -> str:
    revision = _non_empty_string(value, field)
    if not _is_immutable_git_revision(revision):
        raise ValidationError(f"{field} must be a full immutable commit hash")
    return revision


def _immutable_image_ref(value: Any, field: str) -> str:
    image_ref = _non_empty_string(value, field)
    if re.search(r"@sha256:[0-9a-f]{64}$", image_ref) is None:
        raise ValidationError(f"{field} must be an immutable digest image ref")
    return image_ref


def _absolute_path(value: Any, field: str) -> Path:
    raw = _non_empty_string(value, field)
    path = Path(raw)
    if not path.is_absolute() or any(part in ("", ".", "..") for part in path.parts):
        raise ValidationError(f"{field} must be a normalized absolute path")
    return path
