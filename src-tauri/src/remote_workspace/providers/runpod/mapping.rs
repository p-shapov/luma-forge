use crate::{domain::provider::ProviderApiError, remote_workspace::errors::RemoteWorkspaceError};

pub fn not_implemented(operation: &str) -> RemoteWorkspaceError {
    RemoteWorkspaceError::Provider(ProviderApiError::RequestFailed {
        message: format!("RunPod provider operation is not implemented: {operation}"),
    })
}

pub fn bytes_to_runpod_volume_gb(size_bytes: u64) -> u64 {
    size_bytes.div_ceil(1_000_000_000)
}

pub fn workspace_resource_name(workspace_id: &str, suffix: &str) -> String {
    format!("luma-forge-{workspace_id}-{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_to_runpod_volume_gb_rounds_up_to_decimal_gb() {
        assert_eq!(bytes_to_runpod_volume_gb(0), 0);
        assert_eq!(bytes_to_runpod_volume_gb(1), 1);
        assert_eq!(bytes_to_runpod_volume_gb(1_000_000_000), 1);
        assert_eq!(bytes_to_runpod_volume_gb(1_000_000_001), 2);
        assert_eq!(bytes_to_runpod_volume_gb(4_000_000_000), 4);
    }

    #[test]
    fn workspace_resource_name_is_deterministic() {
        assert_eq!(
            workspace_resource_name("workspace-1", "volume"),
            "luma-forge-workspace-1-volume"
        );
    }
}
