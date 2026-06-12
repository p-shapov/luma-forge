use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderApiError {
    Unauthorized,
    InsufficientPermissions,
    RateLimited,
    Timeout,
    RequestFailed,
}
