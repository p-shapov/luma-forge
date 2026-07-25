use luma_diagnostics::DiagnosticDebug;

#[derive(DiagnosticDebug)]
struct Request {
    #[diagnostic(show, redact)]
    value: String,
}

fn main() {}
