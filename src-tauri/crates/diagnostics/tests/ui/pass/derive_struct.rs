use luma_diagnostics::DiagnosticDebug;

#[derive(DiagnosticDebug)]
struct Named {
    #[diagnostic(show)]
    shown: String,
    #[diagnostic(redact)]
    redacted: String,
    omitted: String,
}

#[derive(DiagnosticDebug)]
struct Tuple(#[diagnostic(show)] String, #[diagnostic(redact)] String, String);

#[derive(DiagnosticDebug)]
struct Unit;

fn assert_diagnostic<T: luma_diagnostics::DiagnosticValue>() {}

fn main() {
    assert_diagnostic::<Named>();
    assert_diagnostic::<Tuple>();
    assert_diagnostic::<Unit>();
}
