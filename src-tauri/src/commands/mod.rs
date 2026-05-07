//! Tauri command boundary exposed to the React client.

use tauri_specta::{collect_commands, Builder};

#[cfg(any(debug_assertions, test))]
mod bindings;

#[cfg(any(debug_assertions, test))]
pub(crate) use bindings::export_typescript_bindings;

#[tauri::command]
#[specta::specta]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
#[specta::specta]
fn bye(name: &str) -> String {
    format!("Bye, {}!", name)
}

pub(crate) fn builder() -> Builder<tauri::Wry> {
    Builder::new().commands(collect_commands![greet, bye])
}
