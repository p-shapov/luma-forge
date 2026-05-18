pub(crate) fn provider_resource_name(workspace_id: &str, suffix: &str) -> String {
    format!("luma-forge-{workspace_id}-{suffix}")
}
