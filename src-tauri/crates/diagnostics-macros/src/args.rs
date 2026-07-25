use syn::{parse::Parse, Attribute, Result};

#[derive(Clone, Copy)]
pub enum ValuePolicy {
    Omit,
    Show,
    Redact,
}

pub enum SpanMode {
    Ambient,
    Root,
    Detached,
    Restore(syn::Expr),
}

pub struct FunctionArgs {
    pub span: SpanMode,
    pub output: ValuePolicy,
    pub error: ValuePolicy,
}

impl Default for FunctionArgs {
    fn default() -> Self {
        Self {
            span: SpanMode::Ambient,
            output: ValuePolicy::Omit,
            error: ValuePolicy::Omit,
        }
    }
}

impl Parse for FunctionArgs {
    fn parse(input: syn::parse::ParseStream<'_>) -> Result<Self> {
        let mut args = Self::default();
        let metas = input.parse_terminated(syn::Meta::parse, syn::Token![,])?;

        for meta in metas {
            let path = meta.path();
            if path.is_ident("root") {
                require_path(&meta)?;
                set_span_mode(&mut args.span, SpanMode::Root, &meta)?;
            } else if path.is_ident("detached") {
                require_path(&meta)?;
                set_span_mode(&mut args.span, SpanMode::Detached, &meta)?;
            } else if path.is_ident("restore") {
                let syn::Meta::NameValue(name_value) = &meta else {
                    return Err(syn::Error::new_spanned(
                        meta,
                        "expected `restore = expression`",
                    ));
                };
                set_span_mode(
                    &mut args.span,
                    SpanMode::Restore(name_value.value.clone()),
                    &meta,
                )?;
            } else if path.is_ident("show_output") {
                require_path(&meta)?;
                set_policy(&mut args.output, ValuePolicy::Show, &meta, "output")?;
            } else if path.is_ident("redact_output") {
                require_path(&meta)?;
                set_policy(&mut args.output, ValuePolicy::Redact, &meta, "output")?;
            } else if path.is_ident("show_error") {
                require_path(&meta)?;
                set_policy(&mut args.error, ValuePolicy::Show, &meta, "error")?;
            } else if path.is_ident("redact_error") {
                require_path(&meta)?;
                set_policy(&mut args.error, ValuePolicy::Redact, &meta, "error")?;
            } else {
                return Err(syn::Error::new_spanned(
                    meta,
                    "expected `root`, `detached`, `restore = expression`, `show_output`, `redact_output`, `show_error`, or `redact_error`",
                ));
            }
        }

        Ok(args)
    }
}

fn require_path(meta: &syn::Meta) -> Result<()> {
    if matches!(meta, syn::Meta::Path(_)) {
        Ok(())
    } else {
        Err(syn::Error::new_spanned(
            meta,
            "expected a diagnostic policy flag",
        ))
    }
}

fn set_span_mode(span: &mut SpanMode, next: SpanMode, meta: &syn::Meta) -> Result<()> {
    if !matches!(span, SpanMode::Ambient) {
        return Err(syn::Error::new_spanned(
            meta,
            "duplicate or conflicting diagnostic span mode",
        ));
    }
    *span = next;
    Ok(())
}

fn set_policy(
    policy: &mut ValuePolicy,
    next: ValuePolicy,
    meta: &syn::Meta,
    name: &str,
) -> Result<()> {
    if !matches!(policy, ValuePolicy::Omit) {
        return Err(syn::Error::new_spanned(
            meta,
            format!("duplicate or conflicting diagnostic {name} policy"),
        ));
    }
    *policy = next;
    Ok(())
}

pub fn value_policy(attributes: &[Attribute]) -> Result<ValuePolicy> {
    let mut policy = ValuePolicy::Omit;

    for attribute in attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("diagnostic"))
    {
        attribute.parse_nested_meta(|meta| {
            let next = if meta.path.is_ident("show") {
                ValuePolicy::Show
            } else if meta.path.is_ident("redact") {
                ValuePolicy::Redact
            } else {
                return Err(meta.error("expected `show` or `redact`"));
            };

            if !matches!(policy, ValuePolicy::Omit) {
                return Err(meta.error("duplicate or conflicting diagnostic field policy"));
            }
            policy = next;
            Ok(())
        })?;
    }

    Ok(policy)
}
