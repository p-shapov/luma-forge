#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RunpodError {
    #[error("RunPod request was unauthorized")]
    Unauthorized,
    #[error("RunPod request has insufficient permissions")]
    InsufficientPermissions,
    #[error("RunPod request was rate limited")]
    RateLimited,
    #[error("RunPod request timed out")]
    Timeout,
    #[error("RunPod request failed")]
    RequestFailed,
    #[error("RunPod response was invalid")]
    InvalidResponse,
}
