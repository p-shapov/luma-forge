use luma_diagnostics::diagnostic;

#[derive(Debug)]
struct SafeError;

#[diagnostic(root)]
async fn root() -> Result<(), SafeError> {
    Ok(())
}

#[diagnostic(root)]
fn sync_root() -> Result<(), SafeError> {
    Ok(())
}

fn main() {}
