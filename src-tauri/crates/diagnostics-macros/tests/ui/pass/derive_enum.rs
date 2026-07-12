use luma_diagnostics_macros::DiagnosticDebug;

mod diagnostics {
    pub trait DiagnosticValue: std::fmt::Debug {}

    pub struct Redacted;

    impl std::fmt::Debug for Redacted {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("[REDACTED]")
        }
    }
}

impl diagnostics::DiagnosticValue for String {}

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

fn assert_diagnostic<T: diagnostics::DiagnosticValue>() {}

fn main() {
    assert_diagnostic::<Message>();
}
