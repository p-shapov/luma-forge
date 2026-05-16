use crate::{
    domain::{
        provider_inventory::{Datacenter, GpuOption, ProviderInventory},
        provider_setup::{GpuCloudProviderId, ProviderApiKey, ProviderIdentity},
        workspace::ProviderResourceStatus,
    },
    provider::{
        error::ProviderClientError,
        runpod::contracts::{
            GraphQlError, GraphQlResponse, RunPodApiKey, RunPodEndpointObservation,
            RunPodEndpointResponse, RunPodGpuAvailability, RunPodIdentityData, RunPodInventoryData,
            RunPodNetworkVolumeObservation, RunPodNetworkVolumeResponse, RunPodPodObservation,
            RunPodPodResponse, RunPodTemplateObservation, RunPodTemplateResponse,
        },
    },
};

pub(super) fn identity_from_graphql_response(
    api_key: &ProviderApiKey,
    payload: GraphQlResponse<RunPodIdentityData>,
) -> Result<ProviderIdentity, ProviderClientError> {
    if let Some(errors) = payload.errors.filter(|errors| !errors.is_empty()) {
        return Err(classify_graphql_errors(&errors));
    }

    let myself = payload
        .data
        .and_then(|data| data.myself)
        .ok_or(ProviderClientError::ResponseInvalid)?;
    let provider_user_email = myself
        .email
        .filter(|email| !email.is_empty())
        .ok_or(ProviderClientError::ResponseInvalid)?;
    let api_keys = myself
        .api_keys
        .ok_or(ProviderClientError::ResponseInvalid)?;
    let matched_api_key = match_api_key(api_key.expose_secret(), &api_keys)?;

    if matched_api_key.is_active != Some(true) {
        return Err(ProviderClientError::Unauthorized);
    }

    Ok(ProviderIdentity {
        provider_user_email,
        provider_api_key_fingerprint: matched_api_key
            .id
            .clone()
            .ok_or(ProviderClientError::ResponseInvalid)?,
    })
}

fn match_api_key<'a>(
    secret: &str,
    api_keys: &'a [RunPodApiKey],
) -> Result<&'a RunPodApiKey, ProviderClientError> {
    let mut matches = api_keys
        .iter()
        .filter(|api_key| {
            api_key
                .id
                .as_ref()
                .is_some_and(|id| !id.is_empty() && secret.starts_with(id))
        })
        .take(2);

    let Some(first) = matches.next() else {
        return Err(ProviderClientError::ResponseInvalid);
    };
    if matches.next().is_some() {
        return Err(ProviderClientError::ResponseInvalid);
    }

    Ok(first)
}

fn classify_graphql_errors(errors: &[GraphQlError]) -> ProviderClientError {
    if errors.iter().any(|error| {
        let message = error.message.to_ascii_lowercase();
        message.contains("unauthorized")
            || message.contains("forbidden")
            || message.contains("unauthenticated")
            || message.contains("authentication")
            || message.contains("api key")
    }) {
        ProviderClientError::Unauthorized
    } else {
        ProviderClientError::RequestRejected
    }
}

pub(super) fn inventory_from_graphql_response(
    payload: GraphQlResponse<RunPodInventoryData>,
) -> Result<ProviderInventory, ProviderClientError> {
    if let Some(errors) = payload.errors.filter(|errors| !errors.is_empty()) {
        return Err(classify_graphql_errors(&errors));
    }

    let data_centers = payload
        .data
        .and_then(|data| data.data_centers)
        .ok_or(ProviderClientError::ResponseInvalid)?;

    let mut datacenters = Vec::new();
    for data_center in data_centers {
        if data_center.storage_support != Some(true) {
            continue;
        }

        let id = data_center
            .id
            .filter(|id| !id.is_empty())
            .ok_or(ProviderClientError::ResponseInvalid)?;
        let name = data_center.name.unwrap_or_else(|| id.clone());
        let gpu_options = data_center
            .gpu_availability
            .unwrap_or_default()
            .into_iter()
            .filter_map(gpu_option_from_availability)
            .collect();

        datacenters.push(Datacenter {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            id,
            name,
            gpu_options,
        });
    }

    let fetched_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| ProviderClientError::ResponseInvalid)?;

    Ok(ProviderInventory {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        fetched_at,
        max_persistent_storage_volume_size_bytes: None,
        datacenters,
    })
}

pub(super) fn network_volume_from_response(
    payload: RunPodNetworkVolumeResponse,
) -> Result<RunPodNetworkVolumeObservation, ProviderClientError> {
    Ok(RunPodNetworkVolumeObservation {
        id: non_empty(payload.id)?,
        data_center_id: non_empty(payload.data_center_id)?,
        size_gb: payload.size.ok_or(ProviderClientError::ResponseInvalid)?,
        status: resource_status_or_ready(payload.status.as_deref()),
    })
}

pub(super) fn network_volume_from_list_response(
    payload: RunPodNetworkVolumeResponse,
) -> Result<RunPodNetworkVolumeObservation, ProviderClientError> {
    Ok(RunPodNetworkVolumeObservation {
        id: non_empty(payload.id)?,
        data_center_id: non_empty(payload.data_center_id)?,
        size_gb: payload.size.ok_or(ProviderClientError::ResponseInvalid)?,
        status: resource_status_or_creating(payload.status.as_deref()),
    })
}

#[derive(Debug, Clone)]
pub(super) struct RunPodPodResponseContext {
    pub data_center_id: String,
    pub selected_gpu_id: String,
}

pub(super) fn pod_from_response_with_context(
    payload: RunPodPodResponse,
    context: Option<RunPodPodResponseContext>,
) -> Result<RunPodPodObservation, ProviderClientError> {
    let machine = payload.machine.clone();
    let id = non_empty(payload.id)?;
    Ok(RunPodPodObservation {
        provisioner_status_url: provisioner_status_url(
            &id,
            payload.public_ip,
            payload.ports.unwrap_or_default(),
            payload.port_mappings.unwrap_or_default(),
        ),
        id,
        data_center_id: non_empty(payload.data_center_id.or_else(|| {
            machine
                .as_ref()
                .and_then(|machine| machine.data_center_id.clone())
                .or_else(|| {
                    context
                        .as_ref()
                        .map(|context| context.data_center_id.clone())
                })
        }))?,
        selected_gpu_id: non_empty(payload.gpu_type_id.or_else(|| {
            machine
                .as_ref()
                .and_then(|machine| machine.gpu_type_id.clone())
                .or_else(|| payload.gpu.and_then(|gpu| gpu.id))
                .or_else(|| {
                    context
                        .as_ref()
                        .map(|context| context.selected_gpu_id.clone())
                })
        }))?,
        image_name: non_empty(payload.image_name)?,
        status: resource_status(payload.pod_status.or(payload.desired_status).as_deref()),
    })
}

pub(super) fn template_from_response(
    payload: RunPodTemplateResponse,
) -> Result<RunPodTemplateObservation, ProviderClientError> {
    if payload.is_serverless == Some(false) {
        return Err(ProviderClientError::ResponseInvalid);
    }

    Ok(RunPodTemplateObservation {
        id: non_empty(payload.id)?,
        image_name: non_empty(payload.image_name)?,
        volume_mount_path: non_empty(payload.volume_mount_path)?,
        status: resource_status_or_ready(payload.status.as_deref()),
    })
}

pub(super) fn endpoint_from_response(
    payload: RunPodEndpointResponse,
) -> Result<RunPodEndpointObservation, ProviderClientError> {
    let id = non_empty(payload.id)?;
    let data_center_id = payload
        .data_center_ids
        .unwrap_or_default()
        .into_iter()
        .find(|value| !value.trim().is_empty())
        .ok_or(ProviderClientError::ResponseInvalid)?;
    let selected_gpu_id = payload
        .gpu_type_ids
        .unwrap_or_default()
        .into_iter()
        .find(|value| !value.trim().is_empty())
        .ok_or(ProviderClientError::ResponseInvalid)?;
    let endpoint_invoke_url = payload
        .endpoint_url
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("https://api.runpod.ai/v2/{id}/run"));

    Ok(RunPodEndpointObservation {
        id,
        data_center_id,
        selected_gpu_id,
        status: resource_status_or_ready(payload.status.as_deref()),
        endpoint_invoke_url,
    })
}

fn non_empty(value: Option<String>) -> Result<String, ProviderClientError> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or(ProviderClientError::ResponseInvalid)
}

fn resource_status(status: Option<&str>) -> ProviderResourceStatus {
    match status.unwrap_or_default().to_ascii_uppercase().as_str() {
        "CREATED" | "READY" | "HEALTHY" => ProviderResourceStatus::Ready,
        "RUNNING" | "IN_USE" => ProviderResourceStatus::Running,
        "CREATING" | "PENDING" | "STARTING" | "INITIALIZING" => ProviderResourceStatus::Creating,
        "EXITED" | "STOPPED" | "TERMINATED" | "DELETED" => ProviderResourceStatus::Terminated,
        "FAILED" | "ERROR" | "UNHEALTHY" => ProviderResourceStatus::Failed,
        _ => ProviderResourceStatus::Unknown,
    }
}

fn resource_status_or_ready(status: Option<&str>) -> ProviderResourceStatus {
    match status {
        Some(status) => resource_status(Some(status)),
        None => ProviderResourceStatus::Ready,
    }
}

fn resource_status_or_creating(status: Option<&str>) -> ProviderResourceStatus {
    match status {
        Some(status) => resource_status(Some(status)),
        None => ProviderResourceStatus::Creating,
    }
}

fn provisioner_status_url(
    pod_id: &str,
    public_ip: Option<String>,
    ports: Vec<String>,
    port_mappings: std::collections::HashMap<String, u16>,
) -> Option<String> {
    if let Some(private_port) = exposed_http_port(&ports) {
        return Some(format!(
            "https://{pod_id}-{private_port}.proxy.runpod.net/status"
        ));
    }

    let public_ip = public_ip.filter(|value| !value.trim().is_empty())?;
    let private_port = exposed_tcp_port(&ports).or_else(|| {
        port_mappings
            .keys()
            .find_map(|private_port| private_port.parse::<u16>().ok())
    })?;
    let public_port = port_mappings.get(&private_port.to_string())?;

    Some(format!("http://{public_ip}:{public_port}/status"))
}

fn exposed_http_port(ports: &[String]) -> Option<u16> {
    ports.iter().find_map(|port| {
        let (private_port, protocol) = port.split_once('/')?;
        (protocol.eq_ignore_ascii_case("http")).then_some(private_port.parse::<u16>().ok()?)
    })
}

fn exposed_tcp_port(ports: &[String]) -> Option<u16> {
    ports.iter().find_map(|port| {
        let (private_port, protocol) = port.split_once('/')?;
        (protocol.eq_ignore_ascii_case("tcp")).then_some(private_port.parse::<u16>().ok()?)
    })
}

fn gpu_option_from_availability(availability: RunPodGpuAvailability) -> Option<GpuOption> {
    let gpu_type = availability.gpu_type?;
    let id = gpu_type.id?;
    if id.is_empty() {
        return None;
    }

    let name = gpu_type.display_name.unwrap_or_else(|| id.clone());
    let vram_bytes = gpu_type.memory_in_gb.unwrap_or_default() * 1024 * 1024 * 1024;
    let availability_score = match availability.stock_status.as_deref() {
        Some("High") | Some("HIGH") | Some("Available") | Some("AVAILABLE") => 100,
        Some("Medium") | Some("MEDIUM") => 60,
        Some("Low") | Some("LOW") => 25,
        _ => 0,
    };

    Some(GpuOption {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        id,
        name,
        vram_bytes,
        availability_score,
    })
}
