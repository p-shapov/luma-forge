use luma_diagnostics::diagnostic;

#[diagnostic(detached)]
fn operation() -> Result<(), ()> {
    Ok(())
}

fn main() {}
