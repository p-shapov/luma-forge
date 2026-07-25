use luma_diagnostics::DiagnosticDebug;

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
