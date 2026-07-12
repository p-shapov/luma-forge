use luma_diagnostics_macros::diagnostic;

mod diagnostics {
    pub trait DiagnosticValue: std::fmt::Debug {}
    pub struct Redacted;
    pub struct Fields<'a>(&'a [(&'static str, Field<'a>)]);
    pub enum Field<'a> { Shown(&'a dyn std::fmt::Debug), Redacted }
    impl<'a> Field<'a> {
        pub fn shown<T: DiagnosticValue>(value: &'a T) -> Self { Self::Shown(value) }
        pub const fn redacted() -> Self { Self::Redacted }
    }
    impl<'a> Fields<'a> { pub const fn new(value: &'a [(&'static str, Field<'a>)]) -> Self { Self(value) } }
    impl std::fmt::Debug for Fields<'_> {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { formatter.debug_list().finish() }
    }
    pub fn shown<T: DiagnosticValue>(value: &T) -> &T { value }
}

#[diagnostic(restore = trace_id)]
async fn restore(trace_id: Option<uuid::Uuid>) -> Result<(), ()> { Ok(()) }

fn main() {
    std::mem::drop(restore(Some(uuid::Uuid::new_v4())));
    std::mem::drop(restore(None));
}
