use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DeriveInput, Error, Fields, Lit};

#[proc_macro_derive(DiagnosticCode, attributes(code, diagnostic))]
pub fn derive_diagnostic_code(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_diagnostic_code(&input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand_diagnostic_code(input: &DeriveInput) -> Result<proc_macro2::TokenStream, Error> {
    let ident = &input.ident;
    let Data::Enum(data) = &input.data else {
        return Err(Error::new_spanned(
            input,
            "DiagnosticCode can only be derived for enums",
        ));
    };

    let code_arms = data
        .variants
        .iter()
        .map(|variant| {
            let variant_ident = &variant.ident;
            let source = source_field_index(&variant.fields);
            let code = explicit_diagnostic_code(variant)?
                .map(|code| quote! { #code })
                .or_else(|| source.as_ref().map(|source| source.code_expr.clone()))
                .unwrap_or_else(|| {
                    let code = to_snake_case(&variant.ident.to_string());
                    quote! { #code }
                });
            let pattern = variant_pattern(variant_ident, &variant.fields, source.as_ref());
            Ok(quote! {
                #pattern => #code,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let source_arms = data
        .variants
        .iter()
        .map(|variant| {
            let variant_ident = &variant.ident;
            let source = source_field_index(&variant.fields);
            let pattern = variant_pattern(variant_ident, &variant.fields, source.as_ref());
            let source = source
                .map(|source| {
                    let ident = source.ident;
                    quote! { Some(#ident) }
                })
                .unwrap_or_else(|| quote! { None });

            quote! {
                #pattern => #source,
            }
        })
        .collect::<Vec<_>>();

    Ok(quote! {
        impl crate::diagnostics::HasDiagnosticCode for #ident {
            fn diagnostic_code(&self) -> &'static str {
                match self {
                    #(#code_arms)*
                }
            }

            fn diagnostic_source(&self) -> Option<&dyn crate::diagnostics::HasDiagnosticCode> {
                match self {
                    #(#source_arms)*
                }
            }
        }
    })
}

struct SourceField {
    ident: proc_macro2::TokenStream,
    code_expr: proc_macro2::TokenStream,
}

fn variant_pattern(
    variant_ident: &syn::Ident,
    fields: &Fields,
    source: Option<&SourceField>,
) -> proc_macro2::TokenStream {
    match fields {
        Fields::Unit => quote! { Self::#variant_ident },
        Fields::Unnamed(fields) => {
            let bindings = (0..fields.unnamed.len())
                .map(|index| {
                    let ident = format_ident!("field_{index}");
                    if source
                        .map(|source| source.ident.to_string() == ident.to_string())
                        .unwrap_or(false)
                    {
                        quote! { #ident }
                    } else {
                        quote! { _ }
                    }
                })
                .collect::<Vec<_>>();
            quote! { Self::#variant_ident(#(#bindings),*) }
        }
        Fields::Named(_) => {
            if let Some(source) = source {
                let ident = &source.ident;
                quote! { Self::#variant_ident { #ident, .. } }
            } else {
                quote! { Self::#variant_ident { .. } }
            }
        }
    }
}

fn source_field_index(fields: &Fields) -> Option<SourceField> {
    match fields {
        Fields::Unit => None,
        Fields::Unnamed(fields) => fields
            .unnamed
            .iter()
            .enumerate()
            .find(|(_, field)| is_source_field(field))
            .map(|(index, _)| {
                let ident = format_ident!("field_{index}");
                SourceField {
                    ident: quote! { #ident },
                    code_expr: quote! { #ident.diagnostic_code() },
                }
            }),
        Fields::Named(fields) => fields
            .named
            .iter()
            .find(|field| is_source_field(field))
            .and_then(|field| field.ident.as_ref())
            .map(|ident| SourceField {
                ident: quote! { #ident },
                code_expr: quote! { #ident.diagnostic_code() },
            }),
    }
}

fn is_source_field(field: &syn::Field) -> bool {
    field
        .attrs
        .iter()
        .any(|attribute| attribute.path().is_ident("from") || attribute.path().is_ident("source"))
}

fn explicit_diagnostic_code(variant: &syn::Variant) -> Result<Option<String>, Error> {
    for attribute in &variant.attrs {
        if attribute.path().is_ident("code") {
            let literal = attribute.parse_args::<Lit>()?;
            return string_literal(literal, attribute).map(Some);
        }

        if attribute.path().is_ident("diagnostic") {
            let mut code = None;
            attribute.parse_nested_meta(|meta| {
                if meta.path.is_ident("code") {
                    let value = meta.value()?;
                    let literal = value.parse::<Lit>()?;
                    code = Some(string_literal(literal, attribute)?);
                    Ok(())
                } else {
                    Err(meta.error("unsupported diagnostic attribute"))
                }
            })?;
            if let Some(code) = code {
                return Ok(Some(code));
            }
        }
    }

    Ok(None)
}

fn string_literal(literal: Lit, span: &syn::Attribute) -> Result<String, Error> {
    match literal {
        Lit::Str(value) => Ok(value.value()),
        other => Err(Error::new_spanned(
            other,
            "diagnostic code must be a string literal",
        )
        .into_combine(Error::new_spanned(span, "invalid diagnostic code"))),
    }
}

fn to_snake_case(value: &str) -> String {
    let mut output = String::new();
    let mut previous_was_lower_or_digit = false;

    for character in value.chars() {
        if character.is_ascii_uppercase() {
            if previous_was_lower_or_digit {
                output.push('_');
            }
            output.push(character.to_ascii_lowercase());
            previous_was_lower_or_digit = false;
        } else {
            output.push(character);
            previous_was_lower_or_digit = character.is_ascii_lowercase() || character.is_ascii_digit();
        }
    }

    output
}

trait CombineError {
    fn into_combine(self, other: Error) -> Error;
}

impl CombineError for Error {
    fn into_combine(mut self, other: Error) -> Error {
        self.combine(other);
        self
    }
}
