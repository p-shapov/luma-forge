use luma_diagnostics_macros::diagnostic;

mod diagnostics {
    pub trait DiagnosticValue: std::fmt::Debug {}
    impl DiagnosticValue for String {}
    pub struct Redacted;
    pub enum Field<'a> { Shown(&'a dyn std::fmt::Debug), Redacted }
    impl<'a> Field<'a> {
        pub fn shown<T: DiagnosticValue>(value: &'a T) -> Self { Self::Shown(value) }
        pub const fn redacted() -> Self { Self::Redacted }
    }
    pub struct Fields<'a>(&'a [(&'static str, Field<'a>)]);
    impl<'a> Fields<'a> { pub const fn new(fields: &'a [(&'static str, Field<'a>)]) -> Self { Self(fields) } }
    impl std::fmt::Debug for Fields<'_> {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { formatter.debug_list().finish() }
    }
    pub fn shown<T: DiagnosticValue>(value: &T) -> &T { value }
}

#[derive(Debug)]
struct SafeError;
impl diagnostics::DiagnosticValue for SafeError {}

#[async_trait::async_trait]
trait Port {
    async fn get(&self, id: String) -> Result<String, SafeError>;
}

struct Adapter;

#[diagnostic]
#[async_trait::async_trait]
impl Port for Adapter {
    #[diagnostic(show_output, show_error)]
    async fn get(&self, #[diagnostic(show)] id: String) -> Result<String, SafeError> {
        Ok(id)
    }
}

fn main() {}
