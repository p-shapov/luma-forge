use luma_diagnostics::diagnostic;

#[diagnostic]
fn operation((left, right): (u8, u8)) -> Result<(), ()> {
    Ok(())
}

fn main() {}
