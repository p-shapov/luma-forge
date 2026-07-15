mod args;
mod derive;
mod instrument;

#[proc_macro_attribute]
pub fn diagnostic(
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let args = syn::parse_macro_input!(args as args::FunctionArgs);
    instrument::expand(args, input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(DiagnosticDebug, attributes(diagnostic))]
pub fn derive_diagnostic_debug(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive::expand(syn::parse_macro_input!(input as syn::DeriveInput))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
