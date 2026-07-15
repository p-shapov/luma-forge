use luma_diagnostics_macros::DiagnosticDebug;

mod diagnostics {
    pub trait DiagnosticValue: std::fmt::Debug {}

    pub struct Redacted;
}

#[derive(Debug)]
struct PlainDebug;

#[derive(DiagnosticDebug)]
struct Request {
    #[diagnostic(show)]
    value: PlainDebug,
}

fn main() {}
