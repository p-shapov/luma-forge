mod bundled_catalog;
mod commands;
mod domain;
mod provider;
mod provider_setup;
mod secrets;
mod workspace_catalog;
mod workspace_setup;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = commands::builder();

    #[cfg(debug_assertions)]
    commands::export_typescript_bindings(&builder);

    tauri::Builder::default()
        .manage(provider_setup::ProviderSetupCoordinator::default())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
