use luma_diagnostics_macros::diagnostic;

mod diagnostics {
    use std::fmt::{self, Debug};

    pub trait DiagnosticValue: Debug {}
    impl DiagnosticValue for String {}

    pub struct Redacted;
    impl Debug for Redacted {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("[REDACTED]")
        }
    }

    pub enum Field<'a> {
        Shown(&'a dyn Debug),
        Redacted,
    }
    impl<'a> Field<'a> {
        pub fn shown<T: DiagnosticValue>(value: &'a T) -> Self {
            Self::Shown(value)
        }
        pub const fn redacted() -> Self {
            Self::Redacted
        }
    }
    pub struct Fields<'a>(&'a [(&'static str, Field<'a>)]);
    impl<'a> Fields<'a> {
        pub const fn new(fields: &'a [(&'static str, Field<'a>)]) -> Self {
            Self(fields)
        }
    }
    impl Debug for Fields<'_> {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.debug_list().finish()
        }
    }
    pub fn shown<T: DiagnosticValue>(value: &T) -> &T {
        value
    }
}

struct Unformattable;

#[derive(Debug)]
struct SafeError;
impl diagnostics::DiagnosticValue for SafeError {}

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
