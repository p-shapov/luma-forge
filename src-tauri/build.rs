#[path = "src/infra/bundled/codegen.rs"]
mod bundled_codegen;
#[path = "src/providers/codegen.rs"]
mod provider_codegen;

fn main() {
    bundled_codegen::generate().expect("bundled catalog DTO generation should succeed");
    provider_codegen::generate().expect("provider DTO generation should succeed");
    tauri_build::build()
}
