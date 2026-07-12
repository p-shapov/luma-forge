use luma_diagnostics_macros::DiagnosticDebug;

mod diagnostics {
    pub trait DiagnosticValue: std::fmt::Debug {}

    pub struct Redacted;
}

#[derive(DiagnosticDebug)]
struct Request {
    #[diagnostic(show, redact)]
    value: String,
}

fn main() {}
