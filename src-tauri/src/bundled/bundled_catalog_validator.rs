use std::collections::HashSet;

use crate::{
    bundled::bundled_catalog_error::BundledCatalogError,
    provider::runpod::{RunPodEndpointProfileConfig, RunPodProvisioningProfileConfig},
    workspace::workspace_contracts::{
        ComfyUiRuntimeSource, CustomNodeGitSource, EndpointProfile, ModelAssetSource,
        ProvisioningProfile, WorkflowCatalog,
    },
};

pub(super) fn validate_workflow_catalog(
    catalog: &WorkflowCatalog,
) -> Result<(), BundledCatalogError> {
    if is_blank(&catalog.id) || is_blank(&catalog.version) || catalog.workflow_presets.is_empty() {
        return Err(BundledCatalogError::ValidationFailed);
    }

    let mut ids = HashSet::new();
    for preset in &catalog.workflow_presets {
        if is_blank(&preset.id)
            || is_blank(&preset.version)
            || is_blank(&preset.name)
            || preset.required_base_volume_size_bytes == 0
            || !ids.insert(preset.id.as_str())
        {
            return Err(BundledCatalogError::ValidationFailed);
        }

        for asset in &preset.required_model_assets {
            if is_blank(&asset.id)
                || is_blank(&asset.name)
                || asset.file_size_bytes == 0
                || !is_valid_model_asset_source(&asset.download_source)
                || !is_safe_relative_path(&asset.install.comfyui_relative_path)
            {
                return Err(BundledCatalogError::ValidationFailed);
            }
        }

        if !is_valid_comfyui_source(&preset.required_comfyui_source) {
            return Err(BundledCatalogError::ValidationFailed);
        }

        for node in &preset.required_custom_nodes {
            if is_blank(&node.id)
                || is_blank(&node.name)
                || !is_valid_custom_node_source(&node.git_source)
                || !is_safe_custom_node_path(&node.install.comfyui_custom_nodes_relative_path)
                || !is_optional_safe_relative_path(&node.install.python_requirements_path)
            {
                return Err(BundledCatalogError::ValidationFailed);
            }
        }
    }

    Ok(())
}

pub(super) fn validate_provisioning_profiles(
    profiles: &[ProvisioningProfile],
) -> Result<(), BundledCatalogError> {
    if profiles.is_empty() {
        return Err(BundledCatalogError::ValidationFailed);
    }

    let mut ids = HashSet::new();
    for profile in profiles {
        if is_blank(profile.id()) || !ids.insert(profile.id()) {
            return Err(BundledCatalogError::ValidationFailed);
        }

        match profile {
            ProvisioningProfile::Runpod {
                name,
                version,
                provisioner_worker_runtime,
                gpu_cloud_provider_config,
                ..
            } => {
                if is_blank(name)
                    || is_blank(version)
                    || is_blank(&provisioner_worker_runtime.provisioner_version)
                    || !is_valid_docker_image_ref(&provisioner_worker_runtime.docker_image_ref)
                    || !is_safe_absolute_posix_path(&provisioner_worker_runtime.volume_mount_path)
                    || provisioner_worker_runtime.container_disk_bytes == 0
                    || provisioner_worker_runtime.status_endpoint.port == 0
                    || provisioner_worker_runtime.status_endpoint.protocol != "http"
                    || !is_valid_http_path(&provisioner_worker_runtime.status_endpoint.status_path)
                    || !is_valid_runpod_provisioning_config(
                        gpu_cloud_provider_config,
                        provisioner_worker_runtime.status_endpoint.port,
                    )
                {
                    return Err(BundledCatalogError::ValidationFailed);
                }
            }
        }
    }

    Ok(())
}

pub(super) fn validate_endpoint_profiles(
    profiles: &[EndpointProfile],
) -> Result<(), BundledCatalogError> {
    if profiles.is_empty() {
        return Err(BundledCatalogError::ValidationFailed);
    }

    let mut ids = HashSet::new();
    for profile in profiles {
        if is_blank(profile.id()) || !ids.insert(profile.id()) {
            return Err(BundledCatalogError::ValidationFailed);
        }

        match profile {
            EndpointProfile::Runpod {
                name,
                version,
                endpoint_worker_runtime,
                gpu_cloud_provider_config,
                ..
            } => {
                if is_blank(name)
                    || is_blank(version)
                    || is_blank(&endpoint_worker_runtime.endpoint_worker_version)
                    || !is_valid_docker_image_ref(&endpoint_worker_runtime.docker_image_ref)
                    || endpoint_worker_runtime.http_port == 0
                    || !is_valid_http_path(&endpoint_worker_runtime.health_path)
                    || !is_valid_http_path(&endpoint_worker_runtime.invoke_path)
                    || !is_valid_runpod_endpoint_config(gpu_cloud_provider_config)
                {
                    return Err(BundledCatalogError::ValidationFailed);
                }
            }
        }
    }

    Ok(())
}

fn is_blank(value: &str) -> bool {
    value.trim().is_empty()
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

fn is_safe_relative_path(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() || value.starts_with('/') || value.starts_with('\\') {
        return false;
    }

    value
        .split(['/', '\\'])
        .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
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

fn is_safe_absolute_posix_path(value: &str) -> bool {
    let value = value.trim();
    if value == "/" || !value.starts_with('/') || value.contains('\\') {
        return false;
    }

    value[1..]
        .split('/')
        .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

fn is_valid_http_path(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.starts_with('/')
        && !value.contains('\\')
        && !value.contains('?')
        && !value.contains('#')
        && !value.chars().any(char::is_whitespace)
}

fn is_valid_docker_image_ref(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty()
        || value.contains("://")
        || value.chars().any(char::is_whitespace)
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '.' | '-' | '_' | '/' | ':' | '@')
        })
    {
        return false;
    }

    let Some(first_name_part) = value.split([':', '@']).next() else {
        return false;
    };

    first_name_part.split('/').all(|part| {
        part.chars()
            .any(|character| character.is_ascii_alphanumeric())
    })
}

fn is_valid_runpod_provisioning_config(
    config: &RunPodProvisioningProfileConfig,
    status_port: u16,
) -> bool {
    is_valid_optional_enum(config.cloud_type.as_deref(), &["secure", "community"])
        && is_valid_optional_non_blank(config.pod_template_id.as_deref())
        && is_safe_absolute_posix_path(&config.network_volume_mount_path)
        && !config.expose_http_ports.is_empty()
        && config.expose_http_ports.iter().all(|port| *port != 0)
        && config.expose_http_ports.contains(&status_port)
        && is_valid_environment(config.env.as_ref())
}

fn is_valid_runpod_endpoint_config(config: &RunPodEndpointProfileConfig) -> bool {
    is_valid_optional_non_blank(config.endpoint_template_id.as_deref())
        && config.container_disk_bytes != 0
        && is_safe_absolute_posix_path(&config.volume_mount_path)
        && is_valid_environment(config.env.as_ref())
        && config.scaling.max_workers >= config.scaling.min_workers
        && config.scaling.idle_timeout_seconds != 0
        && is_valid_optional_enum(
            config.scaling.scaler_type.as_deref(),
            &["queue_delay", "request_count"],
        )
        && config.scaling.scaler_value.unwrap_or(1) != 0
}

fn is_valid_optional_enum(value: Option<&str>, allowed: &[&str]) -> bool {
    value
        .map(|value| allowed.contains(&value.trim()))
        .unwrap_or(true)
}

fn is_valid_optional_non_blank(value: Option<&str>) -> bool {
    value.map(|value| !is_blank(value)).unwrap_or(true)
}

fn is_valid_environment(value: Option<&crate::provider::runpod::EnvironmentVariables>) -> bool {
    value
        .map(|environment| {
            environment.iter().all(|(key, value)| {
                !is_blank(key) && !key.contains('=') && !key.contains('\0') && !value.contains('\0')
            })
        })
        .unwrap_or(true)
}
