use std::fmt::{self, Debug};

pub use luma_diagnostics_macros::DiagnosticDebug;

pub trait DiagnosticValue: Debug {}

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
        let mut map = formatter.debug_map();
        for (name, field) in self.0 {
            match field {
                Field::Shown(value) => map.entry(name, value),
                Field::Redacted => map.entry(name, &Redacted),
            };
        }
        map.finish()
    }
}

macro_rules! impl_diagnostic_value {
    ($($type:ty),+ $(,)?) => {
        $(impl DiagnosticValue for $type {})+
    };
}

impl_diagnostic_value!(
    bool,
    char,
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    f32,
    f64,
    str,
    String,
    (),
    uuid::Uuid,
    fastrace::collector::TraceId,
    secrecy::SecretString,
);

impl<T: DiagnosticValue + ?Sized> DiagnosticValue for &T {}
impl<T: DiagnosticValue + ?Sized> DiagnosticValue for &mut T {}
impl<T: DiagnosticValue> DiagnosticValue for Option<T> {}
impl<T: DiagnosticValue> DiagnosticValue for Vec<T> {}
impl<T: DiagnosticValue> DiagnosticValue for [T] {}
impl<T: DiagnosticValue, const N: usize> DiagnosticValue for [T; N] {}

macro_rules! impl_diagnostic_tuple {
    ($($type:ident),+) => {
        impl<$($type: DiagnosticValue),+> DiagnosticValue for ($($type,)+) {}
    };
}

impl_diagnostic_tuple!(A);
impl_diagnostic_tuple!(A, B);
impl_diagnostic_tuple!(A, B, C);
impl_diagnostic_tuple!(A, B, C, D);
impl_diagnostic_tuple!(A, B, C, D, E);
impl_diagnostic_tuple!(A, B, C, D, E, F);

#[cfg(test)]
mod tests;
