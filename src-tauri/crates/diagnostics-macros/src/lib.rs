mod args;
mod derive;

#[proc_macro_derive(DiagnosticDebug, attributes(diagnostic))]
pub fn derive_diagnostic_debug(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive::expand(syn::parse_macro_input!(input as syn::DeriveInput))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
