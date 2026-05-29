use tauri_specta::{collect_commands, Builder};

pub(crate) fn builder() -> Builder<tauri::Wry> {
    Builder::new().commands(collect_commands![
        super::provider_setup::get_gpu_cloud_provider_setup,
        super::provider_setup::setup_gpu_cloud_provider,
        super::provider_setup::delete_gpu_cloud_provider_setup,
        super::hugging_face_setup::get_hugging_face_api_key_setup,
        super::hugging_face_setup::setup_hugging_face_api_key,
        super::hugging_face_setup::delete_hugging_face_api_key_setup,
        super::workspace::get_workflow_catalog,
        super::workspace::get_provider_placement_options,
        super::workspace::get_workspace_catalog,
        super::workspace::create_workspace,
        super::workspace_provisioning::initiate_workspace_provisioning,
        super::workspace_provisioning::sync_workspace_provisioning,
        super::workspace_provisioning::cancel_workspace_provisioning
    ])
}
