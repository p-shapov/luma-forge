mod app_state;
mod bundled_catalog;
mod commands;
mod domain;
mod hugging_face_setup;
mod provider;
mod provider_setup;
mod secrets;
mod workspace_catalog;
mod workspace_provisioning;
mod workspace_resources;
mod workspace_setup;

use tauri::Manager;

const NATIVE_LOG_TARGET_PREFIX: &str = "luma_forge_lib";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = commands::builder();

    #[cfg(debug_assertions)]
    commands::export_typescript_bindings(&builder);

    let mut app_builder = tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .clear_targets()
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("native".to_string()),
                    },
                ))
                .level(log::LevelFilter::Info)
                .filter(|metadata| metadata.target().starts_with(NATIVE_LOG_TARGET_PREFIX))
                .build(),
        )
        .plugin(tauri_plugin_opener::init());

    #[cfg(debug_assertions)]
    {
        app_builder = app_builder.plugin(tauri_plugin_mcp_bridge::init());
    }

    app_builder
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            app.manage(app_state::NativeAppState::new(app.handle().clone()));
            builder.mount_events(app);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::commands;

    #[test]
    fn export_bindings() {
        let builder = commands::builder();

        commands::export_typescript_bindings(&builder);
    }
}
