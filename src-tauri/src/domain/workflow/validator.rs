use std::collections::HashSet;

use crate::domain::{
    error::{DomainValidationError, DomainValidationResult},
    validation::{is_blank, is_safe_relative_path},
};

use super::{ComfyUiRuntimeSource, CustomNodeGitSource, ModelAssetSource, WorkflowCatalog};

pub fn validate_workflow_catalog(catalog: &WorkflowCatalog) -> DomainValidationResult {
    if is_blank(&catalog.id) || is_blank(&catalog.version) || catalog.workflow_presets.is_empty() {
        return Err(DomainValidationError);
    }

    let mut ids = HashSet::new();
    for preset in &catalog.workflow_presets {
        if is_blank(&preset.id)
            || is_blank(&preset.version)
            || is_blank(&preset.name)
            || preset.required_base_volume_size_bytes == 0
            || !ids.insert(preset.id.as_str())
        {
            return Err(DomainValidationError);
        }

        for asset in &preset.required_model_assets {
            if is_blank(&asset.id)
                || is_blank(&asset.name)
                || asset.file_size_bytes == 0
                || !is_valid_model_asset_source(&asset.download_source)
                || !is_safe_relative_path(&asset.install.comfyui_relative_path)
            {
                return Err(DomainValidationError);
            }
        }

        if !is_valid_comfyui_source(&preset.required_comfyui_source) {
            return Err(DomainValidationError);
        }

        for node in &preset.required_custom_nodes {
            if is_blank(&node.id)
                || is_blank(&node.name)
                || !is_valid_custom_node_source(&node.git_source)
                || !is_safe_custom_node_path(&node.install.comfyui_custom_nodes_relative_path)
                || !is_optional_safe_relative_path(&node.install.python_requirements_path)
            {
                return Err(DomainValidationError);
            }
        }
    }

    Ok(())
}

fn is_valid_comfyui_source(source: &ComfyUiRuntimeSource) -> bool {
    match source {
        ComfyUiRuntimeSource::Git {
            repository_url,
            revision,
        } => is_url_shaped(repository_url) && !is_blank(revision),
    }
}

fn is_valid_custom_node_source(source: &CustomNodeGitSource) -> bool {
    match source {
        CustomNodeGitSource::Git {
            repository_url,
            revision,
        } => is_url_shaped(repository_url) && !is_blank(revision),
    }
}

fn is_valid_model_asset_source(source: &ModelAssetSource) -> bool {
    match source {
        ModelAssetSource::Huggingface {
            repository_id,
            file_path,
            revision,
        } => {
            is_huggingface_repository_id(repository_id)
                && is_safe_relative_path(file_path)
                && !is_blank(revision)
        }
    }
}

fn is_safe_custom_node_path(value: &str) -> bool {
    if !is_safe_relative_path(value) {
        return false;
    }

    let mut segments = value.trim().split(['/', '\\']);
    matches!(segments.next(), Some("custom_nodes")) && segments.next().is_some()
}

fn is_optional_safe_relative_path(value: &Option<String>) -> bool {
    value.as_deref().map(is_safe_relative_path).unwrap_or(true)
}

fn is_url_shaped(value: &str) -> bool {
    let value = value.trim();
    let Some((scheme, rest)) = value.split_once("://") else {
        return false;
    };

    !scheme.is_empty()
        && scheme.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        })
        && !rest.is_empty()
        && !rest.chars().any(char::is_whitespace)
        && !rest.starts_with('/')
}

fn is_huggingface_repository_id(value: &str) -> bool {
    let value = value.trim();
    let segments: Vec<_> = value.split('/').collect();
    segments.len() == 2
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && *segment != "."
                && *segment != ".."
                && segment.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
                })
        })
}
