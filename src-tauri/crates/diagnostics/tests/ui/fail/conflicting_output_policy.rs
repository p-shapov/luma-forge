use luma_diagnostics::diagnostic;

#[diagnostic(show_output, redact_output)]
fn operation() -> Result<(), ()> {
    Ok(())
}

fn main() {}
