use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DataEnum, DataStruct, DeriveInput, Fields, Result};

use crate::args::{value_policy, ValuePolicy};

pub fn expand(input: DeriveInput) -> Result<TokenStream> {
    let name = input.ident;
    let generics = input.generics;
    let body = match input.data {
        Data::Struct(data) => struct_body(&name, &data)?,
        Data::Enum(data) => enum_body(&data)?,
        Data::Union(data) => {
            return Err(syn::Error::new_spanned(
                data.union_token,
                "unions are unsupported",
            ))
        }
    };
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics ::std::fmt::Debug for #name #type_generics #where_clause {
            fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                #body
            }
        }

        impl #impl_generics crate::diagnostics::DiagnosticValue for #name #type_generics #where_clause {}
    })
}

fn struct_body(name: &syn::Ident, data: &DataStruct) -> Result<TokenStream> {
    match &data.fields {
        Fields::Named(fields) => {
            let entries = fields
                .named
                .iter()
                .map(|field| {
                    let field_name = field.ident.as_ref().expect("named field");
                    field_entry(
                        value_policy(&field.attrs)?,
                        quote!(&self.#field_name),
                        Some(field_name),
                    )
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(quote! {
                let mut debug = formatter.debug_struct(stringify!(#name));
                #(#entries)*
                debug.finish()
            })
        }
        Fields::Unnamed(fields) => {
            let entries = fields
                .unnamed
                .iter()
                .enumerate()
                .map(|(index, field)| {
                    let index = syn::Index::from(index);
                    field_entry(value_policy(&field.attrs)?, quote!(&self.#index), None)
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(quote! {
                let mut debug = formatter.debug_tuple(stringify!(#name));
                #(#entries)*
                debug.finish()
            })
        }
        Fields::Unit => Ok(quote!(formatter.write_str(stringify!(#name)))),
    }
}

fn enum_body(data: &DataEnum) -> Result<TokenStream> {
    let arms = data
        .variants
        .iter()
        .map(|variant| {
            let variant_name = &variant.ident;
            match &variant.fields {
                Fields::Named(fields) => {
                    let mut bindings = Vec::new();
                    let mut entries = Vec::new();
                    for (index, field) in fields.named.iter().enumerate() {
                        let field_name = field.ident.as_ref().expect("named field");
                        match value_policy(&field.attrs)? {
                            ValuePolicy::Omit => {}
                            ValuePolicy::Show => {
                                let binding = format_ident!("__diagnostic_field_{index}");
                                bindings.push(quote!(#field_name: #binding));
                                entries.push(field_entry(
                                    ValuePolicy::Show,
                                    quote!(#binding),
                                    Some(field_name),
                                )?);
                            }
                            ValuePolicy::Redact => entries.push(field_entry(
                                ValuePolicy::Redact,
                                quote!(()),
                                Some(field_name),
                            )?),
                        }
                    }
                    Ok(quote! {
                        Self::#variant_name { #(#bindings,)* .. } => {
                            let mut debug = formatter.debug_struct(stringify!(#variant_name));
                            #(#entries)*
                            debug.finish()
                        }
                    })
                }
                Fields::Unnamed(fields) => {
                    let mut patterns = Vec::new();
                    let mut entries = Vec::new();
                    for (index, field) in fields.unnamed.iter().enumerate() {
                        let policy = value_policy(&field.attrs)?;
                        match policy {
                            ValuePolicy::Show => {
                                let binding = format_ident!("field_{index}");
                                entries.push(field_entry(
                                    ValuePolicy::Show,
                                    quote!(#binding),
                                    None,
                                )?);
                                patterns.push(quote!(#binding));
                            }
                            ValuePolicy::Omit | ValuePolicy::Redact => {
                                if matches!(policy, ValuePolicy::Redact) {
                                    entries.push(field_entry(
                                        ValuePolicy::Redact,
                                        quote!(()),
                                        None,
                                    )?);
                                }
                                patterns.push(quote!(_));
                            }
                        }
                    }
                    Ok(quote! {
                        Self::#variant_name(#(#patterns),*) => {
                            let mut debug = formatter.debug_tuple(stringify!(#variant_name));
                            #(#entries)*
                            debug.finish()
                        }
                    })
                }
                Fields::Unit => Ok(
                    quote!(Self::#variant_name => formatter.write_str(stringify!(#variant_name))),
                ),
            }
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(quote!(match self { #(#arms),* }))
}

fn field_entry(
    policy: ValuePolicy,
    value: TokenStream,
    name: Option<&syn::Ident>,
) -> Result<TokenStream> {
    let entry = match (policy, name) {
        (ValuePolicy::Omit, _) => TokenStream::new(),
        (ValuePolicy::Show, Some(name)) => quote! {
            assert_diagnostic(#value);
            debug.field(stringify!(#name), #value);
        },
        (ValuePolicy::Show, None) => quote! {
            assert_diagnostic(#value);
            debug.field(#value);
        },
        (ValuePolicy::Redact, Some(name)) => quote! {
            debug.field(stringify!(#name), &crate::diagnostics::Redacted);
        },
        (ValuePolicy::Redact, None) => quote! {
            debug.field(&crate::diagnostics::Redacted);
        },
    };

    if matches!(policy, ValuePolicy::Show) {
        Ok(quote! {
            {
                fn assert_diagnostic<T: crate::diagnostics::DiagnosticValue + ?Sized>(_: &T) {}
                #entry
            }
        })
    } else {
        Ok(entry)
    }
}
