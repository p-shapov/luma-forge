use luma_diagnostics_macros::diagnostic;

#[diagnostic(detached)]
fn operation() -> Result<(), ()> { Ok(()) }

fn main() {}
