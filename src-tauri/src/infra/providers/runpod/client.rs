use std::time::Duration;

use reqwest::StatusCode;
use secrecy::{ExposeSecret, SecretString};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::RunpodError;

const GRAPHQL_URL: &str = "https://api.runpod.io/graphql";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const IDENTITY_QUERY: &str = "query LumaForgeRunpodIdentity { myself { id email } }";
const PLACEMENT_QUERY: &str = r#"query LumaForgeRunpodPlacement {
  gpuTypes {
    id
    displayName
    memoryInGb
  }
  myself {
    datacenters {
      id
      name
      gpuAvailability {
        gpuTypeId
        available
        stockStatus
      }
    }
  }
}"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodIdentity {
    pub user_id: String,
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodPlacementOptions {
    pub gpu_types: Vec<RunpodGpuType>,
    pub datacenters: Vec<RunpodDatacenter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodGpuType {
    pub id: String,
    pub display_name: String,
    pub memory_gb: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodDatacenter {
    pub id: String,
    pub name: String,
    pub gpu_availability: Vec<RunpodGpuAvailability>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodGpuAvailability {
    pub gpu_type_id: String,
    pub available: Option<bool>,
    pub stock_status: Option<String>,
}

#[derive(Clone)]
pub struct RunpodClient {
    http: reqwest::Client,
}

impl RunpodClient {
    pub fn new() -> Result<Self, RunpodError> {
        let http = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|_| RunpodError::RequestFailed)?;

        Ok(Self { http })
    }

    pub async fn identity(&self, api_key: &SecretString) -> Result<RunpodIdentity, RunpodError> {
        map_identity(self.graphql(api_key, IDENTITY_QUERY).await?)
    }

    pub async fn placement_options(
        &self,
        api_key: &SecretString,
    ) -> Result<RunpodPlacementOptions, RunpodError> {
        map_placement(self.graphql(api_key, PLACEMENT_QUERY).await?)
    }

    async fn graphql<T>(
        &self,
        api_key: &SecretString,
        query: &'static str,
    ) -> Result<T, RunpodError>
    where
        T: DeserializeOwned,
    {
        let response = self
            .http
            .post(GRAPHQL_URL)
            .bearer_auth(api_key.expose_secret())
            .json(&GraphqlRequest { query })
            .send()
            .await
            .map_err(transport_error)?;

        if let Some(error) = status_error(response.status()) {
            return Err(error);
        }

        let response = response
            .json::<GraphqlResponse<T>>()
            .await
            .map_err(|_| RunpodError::InvalidResponse)?;

        graphql_data(response)
    }
}

#[derive(Serialize)]
struct GraphqlRequest {
    query: &'static str,
}

#[derive(Deserialize)]
struct GraphqlResponse<T> {
    data: Option<T>,
    #[serde(default)]
    errors: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct IdentityData {
    myself: Option<IdentityUser>,
}

#[derive(Deserialize)]
struct IdentityUser {
    id: String,
    email: String,
}

#[derive(Deserialize)]
struct PlacementData {
    #[serde(rename = "gpuTypes")]
    gpu_types: Vec<GpuTypeData>,
    myself: Option<PlacementUser>,
}

#[derive(Deserialize)]
struct GpuTypeData {
    id: String,
    #[serde(rename = "displayName")]
    display_name: String,
    #[serde(rename = "memoryInGb")]
    memory_gb: u64,
}

#[derive(Deserialize)]
struct PlacementUser {
    datacenters: Vec<DatacenterData>,
}

#[derive(Deserialize)]
struct DatacenterData {
    id: String,
    name: String,
    #[serde(rename = "gpuAvailability")]
    gpu_availability: Vec<GpuAvailabilityData>,
}

#[derive(Deserialize)]
struct GpuAvailabilityData {
    #[serde(rename = "gpuTypeId")]
    gpu_type_id: String,
    available: Option<bool>,
    #[serde(rename = "stockStatus")]
    stock_status: Option<String>,
}

fn graphql_data<T>(response: GraphqlResponse<T>) -> Result<T, RunpodError> {
    if !response.errors.is_empty() {
        return Err(RunpodError::RequestFailed);
    }

    response.data.ok_or(RunpodError::InvalidResponse)
}

fn map_identity(data: IdentityData) -> Result<RunpodIdentity, RunpodError> {
    let identity = data.myself.ok_or(RunpodError::InvalidResponse)?;

    Ok(RunpodIdentity {
        user_id: required(identity.id)?,
        email: required(identity.email)?,
    })
}

fn map_placement(data: PlacementData) -> Result<RunpodPlacementOptions, RunpodError> {
    let user = data.myself.ok_or(RunpodError::InvalidResponse)?;
    let gpu_types = data
        .gpu_types
        .into_iter()
        .map(|gpu| {
            Ok(RunpodGpuType {
                id: required(gpu.id)?,
                display_name: required(gpu.display_name)?,
                memory_gb: gpu.memory_gb,
            })
        })
        .collect::<Result<Vec<_>, RunpodError>>()?;
    let datacenters = user
        .datacenters
        .into_iter()
        .map(|datacenter| {
            let gpu_availability = datacenter
                .gpu_availability
                .into_iter()
                .map(|availability| {
                    Ok(RunpodGpuAvailability {
                        gpu_type_id: required(availability.gpu_type_id)?,
                        available: availability.available,
                        stock_status: normalized(availability.stock_status),
                    })
                })
                .collect::<Result<Vec<_>, RunpodError>>()?;

            Ok(RunpodDatacenter {
                id: required(datacenter.id)?,
                name: required(datacenter.name)?,
                gpu_availability,
            })
        })
        .collect::<Result<Vec<_>, RunpodError>>()?;

    Ok(RunpodPlacementOptions {
        gpu_types,
        datacenters,
    })
}

fn required(value: String) -> Result<String, RunpodError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        Err(RunpodError::InvalidResponse)
    } else {
        Ok(value)
    }
}

fn normalized(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn transport_error(error: reqwest::Error) -> RunpodError {
    if error.is_timeout() {
        RunpodError::Timeout
    } else {
        RunpodError::RequestFailed
    }
}

fn status_error(status: StatusCode) -> Option<RunpodError> {
    if status.is_success() {
        return None;
    }

    Some(match status {
        StatusCode::UNAUTHORIZED => RunpodError::Unauthorized,
        StatusCode::FORBIDDEN => RunpodError::InsufficientPermissions,
        StatusCode::TOO_MANY_REQUESTS => RunpodError::RateLimited,
        _ => RunpodError::RequestFailed,
    })
}

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;
    use serde_json::json;

    use super::*;

    #[test]
    fn maps_a_valid_identity() {
        assert_eq!(
            map_identity(IdentityData {
                myself: Some(IdentityUser {
                    id: " user-id ".to_string(),
                    email: " user@example.com ".to_string(),
                }),
            }),
            Ok(RunpodIdentity {
                user_id: "user-id".to_string(),
                email: "user@example.com".to_string(),
            })
        );
    }

    #[test]
    fn rejects_graphql_errors_and_missing_identity() {
        assert_eq!(
            graphql_data(GraphqlResponse::<IdentityData> {
                data: None,
                errors: vec![json!({})],
            })
            .map(|_| ()),
            Err(RunpodError::RequestFailed)
        );
        assert_eq!(
            map_identity(IdentityData { myself: None }),
            Err(RunpodError::InvalidResponse)
        );
    }

    #[test]
    fn preserves_placement_availability() {
        assert_eq!(
            map_placement(PlacementData {
                gpu_types: vec![GpuTypeData {
                    id: "gpu-1".to_string(),
                    display_name: "GPU One".to_string(),
                    memory_gb: 24,
                }],
                myself: Some(PlacementUser {
                    datacenters: vec![DatacenterData {
                        id: "dc-1".to_string(),
                        name: "Datacenter One".to_string(),
                        gpu_availability: vec![GpuAvailabilityData {
                            gpu_type_id: "gpu-1".to_string(),
                            available: Some(false),
                            stock_status: Some("LOW".to_string()),
                        }],
                    }],
                }),
            }),
            Ok(RunpodPlacementOptions {
                gpu_types: vec![RunpodGpuType {
                    id: "gpu-1".to_string(),
                    display_name: "GPU One".to_string(),
                    memory_gb: 24,
                }],
                datacenters: vec![RunpodDatacenter {
                    id: "dc-1".to_string(),
                    name: "Datacenter One".to_string(),
                    gpu_availability: vec![RunpodGpuAvailability {
                        gpu_type_id: "gpu-1".to_string(),
                        available: Some(false),
                        stock_status: Some("LOW".to_string()),
                    }],
                }],
            })
        );
    }

    #[test]
    fn classifies_http_statuses() {
        assert_eq!(status_error(StatusCode::OK), None);
        assert_eq!(
            status_error(StatusCode::UNAUTHORIZED),
            Some(RunpodError::Unauthorized)
        );
        assert_eq!(
            status_error(StatusCode::FORBIDDEN),
            Some(RunpodError::InsufficientPermissions)
        );
        assert_eq!(
            status_error(StatusCode::TOO_MANY_REQUESTS),
            Some(RunpodError::RateLimited)
        );
        assert_eq!(
            status_error(StatusCode::BAD_GATEWAY),
            Some(RunpodError::RequestFailed)
        );
    }
}
