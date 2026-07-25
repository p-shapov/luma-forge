use luma_diagnostics::diagnostic;

#[derive(Debug)]
struct Plain;

#[diagnostic(show_output)]
async fn operation() -> Result<Plain, ()> {
    loop {}
}

fn main() {}
