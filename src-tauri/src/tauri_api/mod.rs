use tauri_specta::{collect_commands, collect_events, Builder};

pub mod background;
pub mod catalog;
pub(crate) mod diagnostics;
pub mod errors;
pub mod events;
pub mod native;
pub mod secrets;
pub mod support;
pub mod types;
pub mod workspaces;

pub use errors::{
    CommandError, CommandResult, NativeCommandError, NativeInitializationCommandErrorCode,
};

pub fn builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            native::get_native_startup_status,
            catalog::get_workflow_catalog,
            catalog::get_runtime_contract_catalog,
            catalog::get_runpod_placement_options,
            catalog::get_workspace_catalog,
            secrets::setup_runpod_api_key,
            secrets::get_runpod_api_key_identity,
            secrets::delete_runpod_api_key,
            secrets::setup_hugging_face_api_key,
            secrets::get_hugging_face_api_key_identity,
            secrets::delete_hugging_face_api_key,
            workspaces::create_runpod_workspace,
            workspaces::provision_workspace,
            workspaces::cleanup_workspace,
            workspaces::delete_workspace,
            workspaces::get_running_lifecycle_operations,
            workspaces::get_latest_lifecycle_operation
        ])
        .events(collect_events![
            types::workspace::LifecycleOperationChangedEvent,
            types::workspace::WorkspaceChangedEvent,
            types::workspace::WorkspaceDeletedEvent
        ])
}

pub fn export_typescript_bindings(
    builder: &Builder<tauri::Wry>,
) -> Result<(), Box<dyn std::error::Error>> {
    builder.export(
        specta_typescript::Typescript::default(),
        "../src/generated/commands.ts",
    )?;
    Ok(())
}
