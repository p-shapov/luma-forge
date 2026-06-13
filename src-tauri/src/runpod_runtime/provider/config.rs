use super::RunpodEndpointKeepAliveLimits;

pub const RUNPOD_REST_BASE_URL: &str = "https://rest.runpod.io/v1";
pub const RUNPOD_GRAPHQL_URL: &str = "https://api.runpod.io/graphql";
pub const NETWORK_VOLUME_MAX_SIZE_GB: u64 = 4_000;
pub const PROVISIONER_WORKSPACE_MOUNT_PATH: &str = "/workspace";
pub const ENDPOINT_WORKSPACE_MOUNT_PATH: &str = "/runpod-volume";
pub const PROVISIONER_PORT: u16 = 8000;
pub(super) const PROVISIONER_COMPUTE_TYPE: &str = "CPU";
pub(super) const WORKER_PORT_PROTOCOL: &str = "http";
pub(super) const ENDPOINT_WORKERS_MIN: u32 = 0;
pub(super) const ENDPOINT_WORKERS_MAX: u32 = 1;
pub(super) const ENV_PROVISIONER_BEARER_TOKEN: &str = "LUMA_FORGE_PROVISIONER_BEARER_TOKEN";
pub(super) const ENV_PROVISIONER_JOB_ID: &str = "LUMA_FORGE_PROVISIONER_JOB_ID";
pub(super) const ENV_PROVISIONER_REQUIRES_HF_KEY: &str =
    "LUMA_FORGE_PROVISIONER_REQUIRES_HUGGING_FACE_API_KEY";
pub(super) const ENV_PROVISIONER_REQUIRED_MODEL_ASSETS: &str =
    "LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS";
pub(super) const ENV_HUGGING_FACE_API_KEY: &str = "LUMA_FORGE_HUGGING_FACE_API_KEY";
pub(super) const ENV_ENDPOINT_WORKSPACE_MOUNT_PATH: &str =
    "LUMA_FORGE_RUNPOD_ENDPOINT_WORKSPACE_MOUNT_PATH";
pub const DEFAULT_ENDPOINT_KEEP_ALIVE_LIMITS: RunpodEndpointKeepAliveLimits =
    RunpodEndpointKeepAliveLimits {
        default_seconds: 300,
        min_seconds: 0,
        max_seconds: 86_400,
    };
