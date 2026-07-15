mod commands;
mod errors;
mod events;
mod models;
mod state;

pub use commands::*;
pub use errors::*;
pub use events::*;
pub use models::*;
pub use state::*;

pub fn builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![
            commands::get_workflows,
            commands::get_workspaces,
            commands::create_workspace,
            commands::delete_workspace,
            commands::provision_workspace,
            commands::cleanup_workspace,
            commands::get_runtime_operations,
            commands::get_runpod_placement,
            commands::setup_runpod_api_key,
            commands::setup_hugging_face_api_key,
            commands::get_runpod_identity,
            commands::get_hugging_face_identity,
            commands::delete_runpod_api_key,
            commands::delete_hugging_face_api_key,
        ])
        .events(tauri_specta::collect_events![
            WorkspaceChangedEvent,
            WorkspaceDeletedEvent,
            RuntimeOperationEvent,
        ])
}

pub fn export_typescript_bindings(
    builder: &tauri_specta::Builder<tauri::Wry>,
) -> Result<(), specta_typescript::Error> {
    builder.export(
        specta_typescript::Typescript::default(),
        "../src/generated/commands.ts",
    )
}
