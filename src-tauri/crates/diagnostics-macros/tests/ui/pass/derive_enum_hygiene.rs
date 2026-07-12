use luma_diagnostics_macros::DiagnosticDebug;

mod diagnostics {
    pub trait DiagnosticValue: std::fmt::Debug {}

    pub struct Redacted;
}

impl diagnostics::DiagnosticValue for String {}

#[derive(DiagnosticDebug)]
enum Message {
    Named {
        #[diagnostic(show)]
        debug: String,
        #[diagnostic(show)]
        formatter: String,
    },
}

fn main() {}
