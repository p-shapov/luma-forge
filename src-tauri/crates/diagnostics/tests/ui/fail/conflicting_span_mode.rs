use luma_diagnostics::diagnostic;

#[diagnostic(root, detached)]
async fn operation() -> Result<(), ()> {
    Ok(())
}

fn main() {}
