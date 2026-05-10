use crate::{
    domain::{
        provider_inventory::{Datacenter, GpuOption, ProviderInventory},
        provider_setup::{GpuCloudProviderId, ProviderApiKey, ProviderIdentity},
    },
    provider::{
        error::ProviderClientError,
        runpod::contracts::{
            GraphQlError, GraphQlResponse, RunPodApiKey, RunPodGpuAvailability, RunPodIdentityData,
            RunPodInventoryData,
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
        .ok_or(ProviderClientError::IdentityUnavailable)?;
    let provider_user_email = myself
        .email
        .filter(|email| !email.is_empty())
        .ok_or(ProviderClientError::IdentityUnavailable)?;
    let api_keys = myself
        .api_keys
        .ok_or(ProviderClientError::IdentityUnavailable)?;
    let matched_api_key = match_api_key(api_key.expose_secret(), &api_keys)?;

    if matched_api_key.is_active != Some(true) {
        return Err(ProviderClientError::Unauthorized);
    }

    Ok(ProviderIdentity {
        provider_user_email,
        provider_api_key_fingerprint: matched_api_key
            .id
            .clone()
            .ok_or(ProviderClientError::IdentityUnavailable)?,
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
        return Err(ProviderClientError::IdentityUnavailable);
    };
    if matches.next().is_some() {
        return Err(ProviderClientError::IdentityUnavailable);
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
        ProviderClientError::ApiUnavailable
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
        .ok_or(ProviderClientError::ApiUnavailable)?;

    let mut datacenters = Vec::new();
    for data_center in data_centers {
        let id = data_center
            .id
            .filter(|id| !id.is_empty())
            .ok_or(ProviderClientError::ApiUnavailable)?;
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
        .map_err(|_| ProviderClientError::ApiUnavailable)?;

    Ok(ProviderInventory {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        fetched_at,
        max_persistent_storage_volume_size_bytes: None,
        datacenters,
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
