#[path = "src/infra/bundled/codegen.rs"]
mod bundled_codegen;
#[path = "src/infra/clients/codegen.rs"]
mod client_codegen;

fn main() {
    bundled_codegen::generate().expect("bundled catalog DTO generation should succeed");
    client_codegen::generate().expect("provider client DTO generation should succeed");
    tauri_build::build()
}
