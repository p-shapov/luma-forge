use crate::domain::placement::RemoteEndpointKeepAliveLimits;

pub const RUNPOD_REST_BASE_URL: &str = "https://rest.runpod.io/v1";
pub const RUNPOD_GRAPHQL_URL: &str = "https://api.runpod.io/graphql";
pub const NETWORK_VOLUME_MAX_SIZE_BYTES: u64 = 4_000 * 1_000_000_000;
pub const WORKSPACE_MOUNT_PATH: &str = "/workspace";
pub const PROVISIONER_PORT: u16 = 8000;
pub const DEFAULT_ENDPOINT_KEEP_ALIVE_LIMITS: RemoteEndpointKeepAliveLimits =
    RemoteEndpointKeepAliveLimits {
        default_seconds: 300,
        min_seconds: 0,
        max_seconds: 86_400,
    };
