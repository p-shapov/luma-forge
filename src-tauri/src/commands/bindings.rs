//! TypeScript binding export for Tauri commands.

use specta_typescript::Typescript;
use tauri_specta::Builder;

pub(crate) fn export_typescript_bindings(builder: &Builder<tauri::Wry>) {
    let bindings_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated");

    std::fs::create_dir_all(bindings_dir).expect("failed to create generated bindings directory");
    builder
        .export(Typescript::default(), format!("{bindings_dir}/commands.ts"))
        .expect("failed to export typescript bindings");
}
