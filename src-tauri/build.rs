#[path = "build/app_config/mod.rs"]
mod app_config;

fn main() {
    let app_config =
        app_config::AppConfig::from_build_environment().unwrap_or_else(|error| panic!("{error}"));
    app_config.emit_cargo_env();

    tauri_build::build()
}
