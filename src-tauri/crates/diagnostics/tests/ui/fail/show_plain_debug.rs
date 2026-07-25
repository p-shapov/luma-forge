use luma_diagnostics::DiagnosticDebug;

#[derive(Debug)]
struct PlainDebug;

#[derive(DiagnosticDebug)]
struct Request {
    #[diagnostic(show)]
    value: PlainDebug,
}

fn main() {}
