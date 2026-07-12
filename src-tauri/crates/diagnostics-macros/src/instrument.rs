use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    parse_quote, Attribute, Block, FnArg, GenericArgument, ImplItem, ItemFn, ItemImpl, Pat,
    PathArguments, Result, ReturnType, Signature, Type,
};

use crate::args::{value_policy, FunctionArgs, ValuePolicy};

pub fn expand(args: FunctionArgs, input: TokenStream) -> Result<TokenStream> {
    if let Ok(mut function) = syn::parse2::<ItemFn>(input.clone()) {
        instrument_function(&mut function, args)?;
        return Ok(quote!(#function));
    }

    let mut implementation = syn::parse2::<ItemImpl>(input)?;
    if args.root
        || !matches!(args.output, ValuePolicy::Omit)
        || !matches!(args.error, ValuePolicy::Omit)
    {
        return Err(syn::Error::new_spanned(
            &implementation.impl_token,
            "diagnostic policies belong on impl methods",
        ));
    }

    for item in &mut implementation.items {
        let ImplItem::Fn(method) = item else {
            continue;
        };
        let diagnostic_attributes = method
            .attrs
            .iter()
            .filter(|attribute| attribute.path().is_ident("diagnostic"))
            .count();
        if diagnostic_attributes > 1 {
            return Err(syn::Error::new_spanned(
                &method.sig.ident,
                "duplicate diagnostic method policy",
            ));
        }
        let args = if let Some(index) = method
            .attrs
            .iter()
            .position(|attribute| attribute.path().is_ident("diagnostic"))
        {
            parse_attribute_args(&method.attrs.remove(index))?
        } else {
            FunctionArgs::default()
        };
        instrument(&mut method.sig, &mut method.block, &args)?;
        if !args.root {
            method.attrs.push(parse_quote!(#[fastrace::trace]));
        }
    }

    Ok(quote!(#implementation))
}

fn instrument_function(function: &mut ItemFn, args: FunctionArgs) -> Result<()> {
    instrument(&mut function.sig, &mut function.block, &args)?;
    if !args.root {
        function.attrs.push(parse_quote!(#[fastrace::trace]));
    }
    Ok(())
}

fn parse_attribute_args(attribute: &Attribute) -> Result<FunctionArgs> {
    match &attribute.meta {
        syn::Meta::Path(_) => Ok(FunctionArgs::default()),
        syn::Meta::List(list) => syn::parse2(list.tokens.clone()),
        syn::Meta::NameValue(_) => Err(syn::Error::new_spanned(
            attribute,
            "expected diagnostic policy list",
        )),
    }
}

fn instrument(signature: &mut Signature, body: &mut Block, args: &FunctionArgs) -> Result<()> {
    validate_result(&signature.output)?;
    let fields = input_fields(signature)?;
    let fields_ident = syn::Ident::new("__diagnostic_fields", Span::mixed_site());
    let start = if fields.is_empty() {
        quote!(log::info!(function = fastrace::func_path!(); "call.start");)
    } else {
        quote!({
            let #fields_ident = [#(#fields),*];
            log::info!(function = fastrace::func_path!(), input:? = crate::diagnostics::Fields::new(&#fields_ident); "call.start");
        })
    };
    let original = body.clone();
    let result = syn::Ident::new("__diagnostic_result", Span::mixed_site());
    let terminal = terminal_records(&result, args.output, args.error);
    let operation = if signature.asyncness.is_some() {
        quote!((async move #original).await)
    } else {
        quote!((|| #original)())
    };
    let instrumented = quote!({
        #start
        let #result = #operation;
        #terminal
        #result
    });
    let instrumented = if args.root && signature.asyncness.is_some() {
        quote!({
            use fastrace::future::FutureExt as _;
            (async move #instrumented)
                .in_span(fastrace::Span::root(fastrace::func_path!(), fastrace::collector::SpanContext::random()))
                .await
        })
    } else if args.root {
        let root = syn::Ident::new("__diagnostic_root", Span::mixed_site());
        let guard = syn::Ident::new("__diagnostic_guard", Span::mixed_site());
        quote!({
            let #root = fastrace::Span::root(
                fastrace::func_path!(),
                fastrace::collector::SpanContext::random(),
            );
            let #guard = #root.set_local_parent();
            #instrumented
        })
    } else {
        instrumented
    };

    *body = parse_quote!({ #instrumented });
    Ok(())
}

fn validate_result(output: &ReturnType) -> Result<()> {
    let ReturnType::Type(_, ty) = output else {
        return Err(syn::Error::new_spanned(
            output,
            "diagnostic functions must return `Result<T, E>`",
        ));
    };
    let Type::Path(path) = ty.as_ref() else {
        return Err(syn::Error::new_spanned(
            ty,
            "diagnostic functions must return `Result<T, E>`",
        ));
    };
    let Some(segment) = path.path.segments.last() else {
        return Err(syn::Error::new_spanned(
            ty,
            "diagnostic functions must return `Result<T, E>`",
        ));
    };
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(syn::Error::new_spanned(
            ty,
            "diagnostic functions must return `Result<T, E>`",
        ));
    };
    if segment.ident != "Result"
        || arguments.args.len() != 2
        || !arguments
            .args
            .iter()
            .all(|argument| matches!(argument, GenericArgument::Type(_)))
    {
        return Err(syn::Error::new_spanned(
            ty,
            "diagnostic functions must return `Result<T, E>`",
        ));
    }
    Ok(())
}

fn input_fields(signature: &mut Signature) -> Result<Vec<TokenStream>> {
    let mut fields = Vec::new();
    for argument in &mut signature.inputs {
        let FnArg::Typed(argument) = argument else {
            continue;
        };
        let Pat::Ident(pattern) = argument.pat.as_ref() else {
            return Err(syn::Error::new_spanned(
                &argument.pat,
                "diagnostic functions require identifier parameters",
            ));
        };
        let policy = value_policy(&argument.attrs)?;
        argument
            .attrs
            .retain(|attribute| !attribute.path().is_ident("diagnostic"));
        let name = &pattern.ident;
        match policy {
            ValuePolicy::Omit => {}
            ValuePolicy::Show => fields.push(quote! {
                (stringify!(#name), crate::diagnostics::Field::shown(&#name))
            }),
            ValuePolicy::Redact => fields.push(quote! {
                (stringify!(#name), crate::diagnostics::Field::redacted())
            }),
        }
    }
    Ok(fields)
}

fn terminal_records(result: &syn::Ident, output: ValuePolicy, error: ValuePolicy) -> TokenStream {
    let success = match output {
        ValuePolicy::Omit => quote!(log::info!(function = fastrace::func_path!(); "call.success")),
        ValuePolicy::Show => quote!(
            log::info!(function = fastrace::func_path!(), output:? = crate::diagnostics::shown(value); "call.success")
        ),
        ValuePolicy::Redact => quote!(
            log::info!(function = fastrace::func_path!(), output:? = crate::diagnostics::Redacted; "call.success")
        ),
    };
    let failure = match error {
        ValuePolicy::Omit => quote!(
            log::error!(function = fastrace::func_path!(), error_type = std::any::type_name_of_val(error); "call.error")
        ),
        ValuePolicy::Show => quote!(
            log::error!(function = fastrace::func_path!(), error_type = std::any::type_name_of_val(error), error:? = crate::diagnostics::shown(error); "call.error")
        ),
        ValuePolicy::Redact => quote!(
            log::error!(function = fastrace::func_path!(), error_type = std::any::type_name_of_val(error), error:? = crate::diagnostics::Redacted; "call.error")
        ),
    };

    quote! {
        match &#result {
            Ok(value) => #success,
            Err(error) => #failure,
        }
    }
}
