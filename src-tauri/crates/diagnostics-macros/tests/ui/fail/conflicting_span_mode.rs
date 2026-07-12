use luma_diagnostics_macros::diagnostic;

#[diagnostic(root, detached)]
async fn operation() -> Result<(), ()> { Ok(()) }

fn main() {}
