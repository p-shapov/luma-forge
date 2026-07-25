use luma_diagnostics::diagnostic;

#[derive(Debug)]
struct Plain;

#[diagnostic(show_error)]
async fn operation() -> Result<(), Plain> {
    Err(Plain)
}

fn main() {}
