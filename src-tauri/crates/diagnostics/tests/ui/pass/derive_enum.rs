use luma_diagnostics::DiagnosticDebug;

#[derive(DiagnosticDebug)]
enum Message {
    Named {
        #[diagnostic(show)]
        shown: String,
        #[diagnostic(redact)]
        redacted: String,
        omitted: String,
    },
    Tuple(#[diagnostic(show)] String, #[diagnostic(redact)] String, String),
    Unit,
}

fn assert_diagnostic<T: luma_diagnostics::DiagnosticValue>() {}

fn main() {
    assert_diagnostic::<Message>();
}
