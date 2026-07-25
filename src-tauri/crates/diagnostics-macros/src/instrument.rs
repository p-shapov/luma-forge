use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    parse_quote, Attribute, Block, FnArg, GenericArgument, ImplItem, ItemFn, ItemImpl, Pat,
    PathArguments, Result, ReturnType, Signature, Type,
};

use crate::args::{value_policy, FunctionArgs, SpanMode, ValuePolicy};

pub fn expand(args: FunctionArgs, input: TokenStream) -> Result<TokenStream> {
    if let Ok(mut function) = syn::parse2::<ItemFn>(input.clone()) {
        instrument_function(&mut function, args)?;
        return Ok(quote!(#function));
    }

    let mut implementation = syn::parse2::<ItemImpl>(input)?;
    if !matches!(args.span, SpanMode::Ambient)
        || !matches!(args.output, ValuePolicy::Omit)
        || !matches!(args.error, ValuePolicy::Omit)
    {
        return Err(syn::Error::new_spanned(
            implementation.impl_token,
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
    }

    Ok(quote!(#implementation))
}

fn instrument_function(function: &mut ItemFn, args: FunctionArgs) -> Result<()> {
    instrument(&mut function.sig, &mut function.block, &args)?;
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
    if matches!(args.span, SpanMode::Detached) {
        validate_detached(signature)?;
    }
    let fields = input_fields(signature)?;
    let fields_ident = syn::Ident::new("__diagnostic_fields", Span::mixed_site());
    let start = if fields.is_empty() {
        quote!(::luma_diagnostics::__private::log::info!(function = ::luma_diagnostics::__private::fastrace::func_path!(); "call.start");)
    } else {
        quote!({
            let #fields_ident = [#(#fields),*];
            ::luma_diagnostics::__private::log::info!(
                function = ::luma_diagnostics::__private::fastrace::func_path!(),
                input:? = ::luma_diagnostics::__private::Fields::new(&#fields_ident);
                "call.start"
            );
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
    let ambient = quote! {
        ::luma_diagnostics::__private::fastrace::collector::SpanContext::current_local_parent()
            .unwrap_or_else(::luma_diagnostics::__private::fastrace::collector::SpanContext::random)
    };
    let context = match &args.span {
        SpanMode::Ambient | SpanMode::Detached => ambient,
        SpanMode::Root => {
            quote!(::luma_diagnostics::__private::fastrace::collector::SpanContext::random())
        }
        SpanMode::Restore(expression) => quote! {
            (#expression)
                .map(|trace_id: ::luma_diagnostics::__private::uuid::Uuid| {
                    ::luma_diagnostics::__private::fastrace::collector::SpanContext::new(
                        ::luma_diagnostics::__private::fastrace::collector::TraceId(trace_id.as_u128()),
                        ::luma_diagnostics::__private::fastrace::collector::SpanId::default(),
                    )
                })
                .unwrap_or_else(::luma_diagnostics::__private::fastrace::collector::SpanContext::random)
        },
    };

    let instrumented = if matches!(args.span, SpanMode::Detached) {
        let context_ident = syn::Ident::new("__diagnostic_context", Span::mixed_site());
        let output = result_type(&signature.output)?.clone();
        signature.asyncness = None;
        signature.output = parse_quote!(
            -> impl ::std::future::Future<Output = #output> + Send + 'static
        );
        quote!({
            let #context_ident = #context;
            ::luma_diagnostics::__private::fastrace::future::FutureExt::in_span(
                async move #instrumented,
                ::luma_diagnostics::__private::fastrace::Span::root(::luma_diagnostics::__private::fastrace::func_path!(), #context_ident),
            )
        })
    } else if signature.asyncness.is_some() {
        let context_ident = syn::Ident::new("__diagnostic_context", Span::mixed_site());
        quote!({
            let #context_ident = #context;
            ::luma_diagnostics::__private::fastrace::future::FutureExt::in_span(
                async move #instrumented,
                ::luma_diagnostics::__private::fastrace::Span::root(::luma_diagnostics::__private::fastrace::func_path!(), #context_ident),
            )
            .await
        })
    } else {
        let root = syn::Ident::new("__diagnostic_root", Span::mixed_site());
        let guard = syn::Ident::new("__diagnostic_guard", Span::mixed_site());
        quote!({
            let #root = ::luma_diagnostics::__private::fastrace::Span::root(
                ::luma_diagnostics::__private::fastrace::func_path!(),
                #context,
            );
            let #guard = #root.set_local_parent();
            #instrumented
        })
    };

    *body = parse_quote!({ #instrumented });
    Ok(())
}

fn validate_detached(signature: &Signature) -> Result<()> {
    if signature.asyncness.is_none() {
        return Err(syn::Error::new_spanned(
            signature.fn_token,
            "detached diagnostic functions must be async",
        ));
    }
    for input in &signature.inputs {
        let borrowed = match input {
            FnArg::Receiver(receiver) => receiver.reference.is_some(),
            FnArg::Typed(argument) => matches!(argument.ty.as_ref(), Type::Reference(_)),
        };
        if borrowed {
            return Err(syn::Error::new_spanned(
                input,
                "detached diagnostic functions require owned parameters",
            ));
        }
    }
    Ok(())
}

fn result_type(output: &ReturnType) -> Result<&Type> {
    let ReturnType::Type(_, ty) = output else {
        return Err(syn::Error::new_spanned(
            output,
            "diagnostic functions must return `Result<T, E>`",
        ));
    };
    Ok(ty)
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
                (stringify!(#name), ::luma_diagnostics::__private::Field::shown(&#name))
            }),
            ValuePolicy::Redact => fields.push(quote! {
                (stringify!(#name), ::luma_diagnostics::__private::Field::redacted())
            }),
        }
    }
    Ok(fields)
}

fn terminal_records(result: &syn::Ident, output: ValuePolicy, error: ValuePolicy) -> TokenStream {
    let success = match output {
        ValuePolicy::Omit => quote!(
            ::luma_diagnostics::__private::log::info!(function = ::luma_diagnostics::__private::fastrace::func_path!(); "call.success")
        ),
        ValuePolicy::Show => quote!(
            ::luma_diagnostics::__private::log::info!(function = ::luma_diagnostics::__private::fastrace::func_path!(), output:? = ::luma_diagnostics::__private::shown(value); "call.success")
        ),
        ValuePolicy::Redact => quote!(
            ::luma_diagnostics::__private::log::info!(function = ::luma_diagnostics::__private::fastrace::func_path!(), output:? = ::luma_diagnostics::__private::Redacted; "call.success")
        ),
    };
    let failure = match error {
        ValuePolicy::Omit => quote!(
            ::luma_diagnostics::__private::log::error!(function = ::luma_diagnostics::__private::fastrace::func_path!(), error_type = ::std::any::type_name_of_val(error); "call.error")
        ),
        ValuePolicy::Show => quote!(
            ::luma_diagnostics::__private::log::error!(function = ::luma_diagnostics::__private::fastrace::func_path!(), error_type = ::std::any::type_name_of_val(error), error:? = ::luma_diagnostics::__private::shown(error); "call.error")
        ),
        ValuePolicy::Redact => quote!(
            ::luma_diagnostics::__private::log::error!(function = ::luma_diagnostics::__private::fastrace::func_path!(), error_type = ::std::any::type_name_of_val(error), error:? = ::luma_diagnostics::__private::Redacted; "call.error")
        ),
    };

    quote! {
        match &#result {
            Ok(value) => #success,
            Err(error) => #failure,
        }
    }
}
