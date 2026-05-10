use std::collections::HashSet;

use crate::domain::{
    validation_error::{DomainValidationError, DomainValidationResult},
    validation_support::{
        is_blank, is_safe_absolute_posix_path, is_valid_docker_image_ref, is_valid_environment,
        is_valid_http_path, is_valid_optional_enum, is_valid_optional_non_blank,
    },
};

use super::{
    EndpointProfile, ProvisioningProfile, RunPodEndpointProfileConfig,
    RunPodProvisioningProfileConfig,
};

pub fn validate_provisioning_profiles(profiles: &[ProvisioningProfile]) -> DomainValidationResult {
    if profiles.is_empty() {
        return Err(DomainValidationError);
    }

    let mut ids = HashSet::new();
    for profile in profiles {
        if is_blank(profile.id()) || !ids.insert(profile.id()) {
            return Err(DomainValidationError);
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
                    return Err(DomainValidationError);
                }
            }
        }
    }

    Ok(())
}

pub fn validate_endpoint_profiles(profiles: &[EndpointProfile]) -> DomainValidationResult {
    if profiles.is_empty() {
        return Err(DomainValidationError);
    }

    let mut ids = HashSet::new();
    for profile in profiles {
        if is_blank(profile.id()) || !ids.insert(profile.id()) {
            return Err(DomainValidationError);
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
                    return Err(DomainValidationError);
                }
            }
        }
    }

    Ok(())
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

#[cfg(test)]
mod tests {
    use crate::domain::{
        profiles::{
            EndpointWorkerRuntime, ProvisionerWorkerRuntime, ProvisioningComputeType,
            ProvisioningStatusEndpoint, RunPodServerlessScalingConfig,
        },
        workflow::WorkflowExecutionType,
    };

    use super::*;

    #[test]
    fn rejects_invalid_provisioning_environment_key() {
        let mut profile = valid_provisioning_profile();
        let ProvisioningProfile::Runpod {
            gpu_cloud_provider_config,
            ..
        } = &mut profile;
        gpu_cloud_provider_config.env = Some([("BAD=KEY".to_string(), "value".to_string())].into());

        let error = validate_provisioning_profiles(&[profile])
            .expect_err("invalid environment key should fail");

        assert_eq!(error, DomainValidationError);
    }

    #[test]
    fn rejects_inconsistent_endpoint_scaling() {
        let mut profile = valid_endpoint_profile();
        let EndpointProfile::Runpod {
            gpu_cloud_provider_config,
            ..
        } = &mut profile;
        gpu_cloud_provider_config.scaling.min_workers = 2;
        gpu_cloud_provider_config.scaling.max_workers = 1;

        let error =
            validate_endpoint_profiles(&[profile]).expect_err("invalid scaling should fail");

        assert_eq!(error, DomainValidationError);
    }

    fn valid_provisioning_profile() -> ProvisioningProfile {
        ProvisioningProfile::Runpod {
            id: "provisioning".to_string(),
            version: "1.0.0".to_string(),
            name: "Provisioning".to_string(),
            provisioner_worker_runtime: ProvisionerWorkerRuntime {
                provisioner_version: "1.0.0".to_string(),
                docker_image_ref: "ghcr.io/luma-forge/provisioner:1.0.0".to_string(),
                volume_mount_path: "/workspace".to_string(),
                container_disk_bytes: 1,
                compute_type: ProvisioningComputeType::Pod,
                status_endpoint: ProvisioningStatusEndpoint {
                    port: 8000,
                    protocol: "http".to_string(),
                    status_path: "/status".to_string(),
                },
            },
            gpu_cloud_provider_config: RunPodProvisioningProfileConfig {
                cloud_type: None,
                pod_template_id: None,
                network_volume_mount_path: "/workspace".to_string(),
                expose_http_ports: vec![8000],
                env: None,
            },
        }
    }

    fn valid_endpoint_profile() -> EndpointProfile {
        EndpointProfile::Runpod {
            id: "endpoint".to_string(),
            version: "1.0.0".to_string(),
            name: "Endpoint".to_string(),
            workflow_execution_type: WorkflowExecutionType::T2i,
            endpoint_worker_runtime: EndpointWorkerRuntime {
                endpoint_worker_version: "1.0.0".to_string(),
                docker_image_ref: "ghcr.io/luma-forge/endpoint:1.0.0".to_string(),
                http_port: 8188,
                health_path: "/health".to_string(),
                invoke_path: "/prompt".to_string(),
            },
            gpu_cloud_provider_config: RunPodEndpointProfileConfig {
                endpoint_template_id: None,
                container_disk_bytes: 1,
                volume_mount_path: "/workspace".to_string(),
                env: None,
                scaling: RunPodServerlessScalingConfig {
                    min_workers: 0,
                    max_workers: 1,
                    idle_timeout_seconds: 60,
                    scaler_type: None,
                    scaler_value: None,
                },
            },
        }
    }
}
