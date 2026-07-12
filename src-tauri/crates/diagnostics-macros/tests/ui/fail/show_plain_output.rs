use luma_diagnostics_macros::diagnostic;

mod diagnostics {
    pub trait DiagnosticValue: std::fmt::Debug {}
    pub struct Redacted;
    pub enum Field<'a> { Shown(&'a dyn std::fmt::Debug), Redacted }
    impl<'a> Field<'a> {
        pub fn shown<T: DiagnosticValue>(value: &'a T) -> Self { Self::Shown(value) }
        pub const fn redacted() -> Self { Self::Redacted }
    }
    pub struct Fields<'a>(&'a [(&'static str, Field<'a>)]);
    impl<'a> Fields<'a> { pub const fn new(fields: &'a [(&'static str, Field<'a>)]) -> Self { Self(fields) } }
    pub fn shown<T: DiagnosticValue>(value: &T) -> &T { value }
}

#[derive(Debug)]
struct Plain;

#[diagnostic(show_output)]
async fn operation() -> Result<Plain, ()> { loop {} }

fn main() {}
