use luma_diagnostics_macros::diagnostic;

#[diagnostic(show_output, redact_output)]
fn operation() -> Result<(), ()> { Ok(()) }

fn main() {}
