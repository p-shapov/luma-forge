mod app_state;
mod bundled_catalog;
mod commands;
mod domain;
mod provider;
mod provider_setup;
mod secrets;
mod workspace_catalog;
mod workspace_setup;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = commands::builder();

    #[cfg(debug_assertions)]
    commands::export_typescript_bindings(&builder);

    let mut app_builder = tauri::Builder::default().plugin(tauri_plugin_opener::init());

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
