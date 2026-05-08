use tauri_specta::{collect_commands, Builder};

use super::{provider_setup::provider_setup_handlers, workspace::workspace_handlers};

pub(crate) fn builder() -> Builder<tauri::Wry> {
    Builder::new().commands(collect_commands![
        provider_setup_handlers::get_gpu_cloud_provider_setup,
        provider_setup_handlers::setup_gpu_cloud_provider,
        provider_setup_handlers::delete_gpu_cloud_provider_setup,
        workspace_handlers::get_workflow_catalog,
        workspace_handlers::get_provisioning_profiles,
        workspace_handlers::get_endpoint_profiles,
        workspace_handlers::get_provider_inventory,
        workspace_handlers::get_workspace_catalog,
        workspace_handlers::create_workspace
    ])
}
