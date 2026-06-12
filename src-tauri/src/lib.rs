use tauri_specta::{collect_commands, collect_events, Builder};

pub mod app;
pub mod commands;
pub mod domain;
pub mod lifecycle_journal;
pub mod provisioned_remote;
pub mod secrets_storage;
pub mod shared;
pub mod sqlite;
pub mod workflow_catalog;
pub mod workspace_catalog;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = command_builder();

    #[cfg(debug_assertions)]
    export_typescript_bindings(&builder);

    let mut app_builder = tauri::Builder::default().plugin(tauri_plugin_opener::init());

    #[cfg(debug_assertions)]
    {
        app_builder = app_builder.plugin(tauri_plugin_mcp_bridge::init());
    }

    app_builder
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let app_state =
                tauri::async_runtime::block_on(app::bootstrap::build_app_state(&app_handle))
                    .map_err(|error| Box::<dyn std::error::Error>::from(error.message))?;
            tauri::Manager::manage(app, app_state);
            builder.mount_events(app);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn command_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::catalog::get_workflow_catalog,
            commands::catalog::get_runpod_placement_options,
            commands::catalog::get_workspace_catalog,
            commands::secrets::setup_runpod_api_key,
            commands::secrets::get_runpod_api_key_identity,
            commands::secrets::delete_runpod_api_key,
            commands::secrets::setup_hugging_face_api_key,
            commands::secrets::get_hugging_face_api_key_identity,
            commands::secrets::delete_hugging_face_api_key,
            commands::workspaces::create_runpod_workspace,
            commands::workspaces::provision_workspace,
            commands::workspaces::cleanup_workspace,
            commands::workspaces::delete_workspace,
            commands::workspaces::get_running_lifecycle_operations,
            commands::workspaces::get_latest_lifecycle_operation
        ])
        .events(collect_events![
            commands::types::workspace::LifecycleOperationChangedEvent,
            commands::types::workspace::WorkspaceChangedEvent,
            commands::types::workspace::WorkspaceDeletedEvent
        ])
}

fn export_typescript_bindings(builder: &Builder<tauri::Wry>) {
    builder
        .export(
            specta_typescript::Typescript::default(),
            "../src/generated/commands.ts",
        )
        .expect("failed to export TypeScript command bindings");
}

#[cfg(test)]
mod tests {
    use super::{command_builder, export_typescript_bindings};

    #[test]
    fn export_bindings() {
        let builder = command_builder();

        export_typescript_bindings(&builder);
    }
}
