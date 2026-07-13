#[path = "src/infra/bundled/codegen.rs"]
mod bundled_codegen;

fn main() {
    bundled_codegen::generate().expect("bundled catalog DTO generation should succeed");
    tauri_build::build()
}
