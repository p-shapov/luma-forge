use luma_diagnostics::diagnostic;

struct Unformattable;

#[derive(Debug)]
struct SafeError;
impl luma_diagnostics::DiagnosticValue for SafeError {}

#[diagnostic(show_output, show_error)]
async fn operation(
    #[diagnostic(show)] id: String,
    #[diagnostic(redact)] token: Unformattable,
    omitted: Unformattable,
) -> Result<String, SafeError> {
    let (__diagnostic_fields, __diagnostic_result, __diagnostic_value) = (token, omitted, ());
    Ok(id)
}

fn main() {}
