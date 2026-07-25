use luma_diagnostics::diagnostic;

#[derive(Debug)]
struct Plain;

#[diagnostic]
async fn operation(#[diagnostic(show)] value: Plain) -> Result<(), ()> {
    Ok(())
}

fn main() {}
